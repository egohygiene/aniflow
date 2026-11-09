# Redacted metadata audit [skip ci]

`delivery_metadata::audit::from_receipt` creates bounded change counts from an
explicit private before/after receipt and selected stream mapping. The SDK
`observe_remux` instead reads the owning current accepted native remux report,
rechecks it, and labels that authority separately. Neither operation runs tools.

Results contain closed scope, field-category and change enums plus counts;
arbitrary tag names, values, private paths, URLs and hashes of removed values
are excluded. Global/selected video/audio, chapter/program and unselected
stream dictionaries remain distinct. Added, removed, retained and rewritten
fields are separate. Sensitive/history categorization is heuristic and conveys
no legal/privacy guarantee. It does not discover opaque container fields.

A supplied receipt is always labeled supplied. Current retained native
acceptance does not mean fresh decode, universal sanitation, watermark/C2PA
verification, platform survival, creative approval or release qualification.
The existing private receipt remains the detailed owner; no policy or native
completion state is changed. The existing source and clean master are retained.

Five regression cases authored, uncompiled/unrun. All tests/builds/checks/native
and CI execution remain deferred to flow #88, scheduled last. #112 retains
broader metadata/container/qualification criteria. Known alignment cancellation/
start-marker failure remains unresolved; cause unproven.
