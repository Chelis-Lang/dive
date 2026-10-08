# Provenance

Every algorithm ported or adapted from another implementation is recorded here. Each entry gives:
- the `dive` component;
- the source project, file and class or method, and the version or commit;
- the source's licence;
- the nature of the port: semantics matched, or algorithm adapted.

The canonical tlaplus tools are MIT-licensed. Translated code carries attribution.

| dive component | Source | Licence | Nature |
|---|---|---|---|
| `dive-conformance` `tlc/codes.rs` (`EC_CODES`, `exit_statuses_for`, `EXIT_STATUSES`) | tlaplus/tlaplus `tlc2/output/EC.java` (constants, `EC.ExitStatus`, `errorConstantToExitStatus`) at `5a47802` (v1.7.4) and `94d0c50` | MIT | Data transcribed (codes, names, exit mapping); used to classify oracle output, not ported behaviour |
| `dive-conformance` `tlc/messages.rs` | tlaplus/tlaplus `tlc2/output/MP.java` (`-tool` message delimiters and classes) | MIT | Format matched (parser written from the format) |

No checker algorithm has been ported yet.
