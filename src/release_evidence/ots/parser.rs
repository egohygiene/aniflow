//! Closed, bounded parser; computes commitment operations rather than trusting labels.
use super::*;
use std::collections::BTreeSet;
pub(super) enum Failure {
    Invalid(&'static str),
    Unsupported {
        feature: UnsupportedFeature,
        subject: Option<[u8; 32]>,
    },
    Cancelled,
}
type ParsedResult<T> = std::result::Result<T, Failure>;
fn bad(message: &'static str) -> Failure {
    Failure::Invalid(message)
}
pub(super) struct Parsed {
    pub subject: [u8; 32],
    pub operations: u32,
    pub work_bytes: u64,
    pub attestations: Vec<Attestation>,
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> ParsedResult<&'a [u8]> {
        let end = self
            .offset
            .checked_add(count)
            .filter(|n| *n <= self.bytes.len())
            .ok_or_else(|| bad("truncated OTS field"))?;
        let bytes = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> ParsedResult<u8> {
        Ok(self.take(1)?[0])
    }
    fn uint(&mut self) -> ParsedResult<u64> {
        let mut value = 0u64;
        for index in 0..9 {
            let byte = self.byte()?;
            let low = u64::from(byte & 0x7f);
            value |= low << (index * 7);
            if byte & 0x80 == 0 {
                if index > 0 && low == 0 {
                    return Err(bad(
                        "OTS varuint must use its minimal encoding in this profile",
                    ));
                }
                return Ok(value);
            }
        }
        Err(bad("OTS varuint exceeds 63 bits"))
    }
    fn variable(&mut self, minimum: usize, maximum: usize) -> ParsedResult<&'a [u8]> {
        let count = self.uint()?;
        if count < minimum as u64 || count > maximum as u64 {
            return Err(bad("OTS variable field exceeds its finite profile bounds"));
        }
        self.take(count as usize)
    }
    fn eof(&self) -> ParsedResult<()> {
        if self.offset != self.bytes.len() {
            return Err(bad("OTS document or attestation has trailing bytes"));
        }
        Ok(())
    }
}
struct Parser<'a> {
    reader: Reader<'a>,
    token: &'a CancellationToken,
    subject: [u8; 32],
    operations: u32,
    work: u64,
    attestations: Vec<Attestation>,
}
impl Parser<'_> {
    fn cancellation(&self) -> ParsedResult<()> {
        if self.token.is_cancelled() {
            Err(Failure::Cancelled)
        } else {
            Ok(())
        }
    }
    fn attestation(
        &mut self,
        message: &[u8],
        path: &Sha256,
        seen: &mut BTreeSet<(Vec<u8>, Vec<u8>)>,
    ) -> ParsedResult<()> {
        self.cancellation()?;
        if self.attestations.len() >= MAXIMUM_ATTESTATIONS {
            return Err(bad("OTS attestation count exceeds 128"));
        }
        let tag = self.reader.take(8)?;
        let payload = self.reader.variable(0, 8192)?;
        if !seen.insert((tag.to_vec(), payload.to_vec())) {
            return Err(bad(
                "duplicate OTS attestation in one node is outside this profile",
            ));
        }
        let mut reader = Reader {
            bytes: payload,
            offset: 0,
        };
        let detail = if tag == PENDING.as_slice() {
            let uri = reader.variable(1, 1000)?;
            if uri
                .iter()
                .any(|b| !b.is_ascii_alphanumeric() && !b"-._/:".contains(b))
            {
                return Err(bad("OTS pending URI has unsupported characters"));
            }
            reader.eof()?;
            AttestationDetail::PendingCalendar {
                uri_sha256: sha(uri),
                uri_byte_count: uri.len() as u32,
            }
        } else if tag == BITCOIN.as_slice() {
            let height = reader.uint()?;
            reader.eof()?;
            if height > u32::MAX as u64 || message.len() != 32 {
                return Err(bad(
                    "OTS Bitcoin claim needs a 32-byte commitment and uint32 height",
                ));
            }
            AttestationDetail::BitcoinBlockHeader {
                claimed_block_height: height as u32,
                claimed_merkle_root_hex: hex(message),
            }
        } else {
            // Unknown attestations remain opaque. Never interpret them as anchors.
            AttestationDetail::Unknown {
                tag_hex: hex(tag),
                payload_sha256: sha(payload),
                payload_byte_count: payload.len() as u32,
            }
        };
        self.attestations.push(Attestation {
            operation_path_sha256: hex(&path.clone().finalize()),
            commitment_sha256: sha(message),
            commitment_byte_count: message.len() as u32,
            detail,
        });
        Ok(())
    }
    fn operation(
        &mut self,
        tag: u8,
        message: &[u8],
        path: &Sha256,
        seen: &mut BTreeSet<(u8, Vec<u8>)>,
        depth: usize,
    ) -> ParsedResult<()> {
        let argument = match tag {
            0xf0 | 0xf1 => self.reader.variable(1, MAXIMUM_MESSAGE_BYTES)?.to_vec(),
            0x08 | 0xf2 | 0xf3 => vec![],
            _ => {
                return Err(Failure::Unsupported {
                    feature: UnsupportedFeature {
                        kind: UnsupportedKind::CommitmentOperation,
                        tag,
                    },
                    subject: Some(self.subject),
                });
            }
        };
        if !seen.insert((tag, argument.clone())) {
            return Err(bad("duplicate OTS operation in one node"));
        }
        self.operations = self
            .operations
            .checked_add(1)
            .filter(|n| *n <= MAXIMUM_OPERATIONS)
            .ok_or_else(|| bad("OTS operations exceed 4096"))?;
        let length = (match tag {
            0xf0 | 0xf1 => message.len().checked_add(argument.len()),
            0xf3 => message.len().checked_mul(2),
            0x08 => Some(32),
            _ => Some(message.len()),
        })
        .filter(|n| *n > 0 && *n <= MAXIMUM_MESSAGE_BYTES)
        .ok_or_else(|| bad("OTS commitment message exceeds 4096 bytes"))?;
        self.work = self
            .work
            .checked_add(message.len() as u64)
            .and_then(|n| n.checked_add(length as u64))
            .filter(|n| *n <= MAXIMUM_WORK_BYTES)
            .ok_or_else(|| bad("OTS commitment work exceeds 32 MiB"))?;
        let result: Vec<u8> = match tag {
            0x08 => Sha256::digest(message).to_vec(),
            0xf0 => [message, argument.as_slice()].concat(),
            0xf1 => [argument.as_slice(), message].concat(),
            0xf2 => message.iter().rev().copied().collect(),
            0xf3 => hex(message).into_bytes(),
            _ => return Err(bad("unreachable unsupported OTS operation")),
        };
        let mut next = path.clone();
        next.update([tag]);
        next.update((argument.len() as u64).to_be_bytes());
        next.update(&argument);
        self.node(&result, &next, depth + 1)
    }
    fn node(&mut self, message: &[u8], path: &Sha256, depth: usize) -> ParsedResult<()> {
        self.cancellation()?;
        if depth > MAXIMUM_DEPTH {
            return Err(bad("OTS commitment depth exceeds 64"));
        }
        let mut seen_ops = BTreeSet::new();
        let mut seen_attestations = BTreeSet::new();
        for _ in 0..MAXIMUM_NODE_ENTRIES {
            self.cancellation()?;
            let first = self.reader.byte()?;
            let more = first == 0xff;
            let tag = if more { self.reader.byte()? } else { first };
            if tag == 0xff {
                return Err(bad("OTS entry has consecutive branch markers"));
            }
            if tag == 0x00 {
                self.attestation(message, path, &mut seen_attestations)?;
            } else {
                self.operation(tag, message, path, &mut seen_ops, depth)?;
            }
            if !more {
                return Ok(());
            }
        }
        Err(bad("OTS node entries exceed 64"))
    }
}
pub(super) fn parse<'a>(bytes: &'a [u8], token: &'a CancellationToken) -> ParsedResult<Parsed> {
    if token.is_cancelled() {
        return Err(Failure::Cancelled);
    }
    let mut reader = Reader { bytes, offset: 0 };
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(bad("OTS detached magic does not match"));
    }
    let version = reader.byte()?;
    if version != 1 {
        return Err(Failure::Unsupported {
            feature: UnsupportedFeature {
                kind: UnsupportedKind::MajorVersion,
                tag: version,
            },
            subject: None,
        });
    }
    let hash = reader.byte()?;
    if hash != 0x08 {
        return Err(Failure::Unsupported {
            feature: UnsupportedFeature {
                kind: UnsupportedKind::FileHashOperation,
                tag: hash,
            },
            subject: None,
        });
    }
    let subject: [u8; 32] = reader
        .take(32)?
        .try_into()
        .map_err(|_| bad("OTS SHA-256 header length differs"))?;
    let mut parser = Parser {
        reader,
        token,
        subject,
        operations: 0,
        work: 0,
        attestations: vec![],
    };
    let mut path = Sha256::new();
    path.update(b"aniflow.ots-operation-path/v1\0");
    parser.node(&subject, &path, 0)?;
    parser.reader.eof()?;
    Ok(Parsed {
        subject,
        operations: parser.operations,
        work_bytes: parser.work,
        attestations: parser.attestations,
    })
}
