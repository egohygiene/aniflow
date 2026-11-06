# Exact audio extent checkpoint — November 6, 2026

Adds selected preserve_exact audio extent policy to shared temporal admission,
original-index/extraction selection, assembly observation and native output
validation. Strict legacy admission remains the omitted default. Video clocks,
stream/packet identity, AAC-LC passthrough and full decode stay authoritative.
Assembly emits rational offset/tail receipts. The public read-only #13 signal
view retains normalized quantities and unavailable reasons; it explicitly does
not establish a video/PCM relationship or normalize a mix.

Four temporal/loudness regression cases are authored, uncompiled/unrun. Native positive
and negative audio offsets, long/short tails, priming, exact remux, interruption
and accepted loudness-view qualification remain #64. All execution, compilation,
checks and CI remain deferred; the known alignment cancellation/start-marker
failure remains unresolved with unproven cause. No dependencies or releases.

#35 stays open for automatic source-bound PCM/meter composition, additional
supported media/codecs, explicit alternative transforms and complete synthetic/
native workflow qualification. #112 owns selected metadata sanitation and #113
owns read-only completion handoff; these are separate source checkpoints.
