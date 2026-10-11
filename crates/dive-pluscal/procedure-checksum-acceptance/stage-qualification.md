# Stage correction retained before production

The initial next-package proposal called the header serializer post-Explode. The actual pinned PCalTLAGenerator.translate creates a separate local xast and keeps the original ast; trans hashes the original after generation. DefinitionProbe records that original parse/name-fixed/post-generation object, which still contains Call/Return nodes. ProcedureExplodeProbe is a separate development-only fresh parse/name-fix/direct-Explode study; its xast is never treated as the header CRC source.

An initial inference attributed proceduresCalled mutation to generation. Complete parse/name/translated readback disproves it: the simple main call already adds Put to the sole procedure vector at parse stage. GetProcedure stores the mutable static proceduresCalled vector, and GetCall in main appends to the same vector. All stage bytes and the erroneous first inference remain documented separately. No new default behavior is adopted.

## Shared nested vectors

The first native comparison retained six mismatches (`if_call`, `with_call`,
`either_call`, C and P). `Explode` preserves top-level source transfer nodes but
mutates nested branch vectors shared with the original AST. Their translated
header AST therefore contains synthetic assignments. This increment refuses
nested call/return checksum shapes; the raw generator remains a separate stage.
Two retained `local_update` sources are rejected by the primary for a missing
label while the existing native lowering accepts them. The checksum serializer
conservatively refuses more than one sequential assignment node per procedure
label rather than crediting those sources as compatible artifacts.
