---- MODULE StringOrderPreinterned ----
\* Probe: as StringOrderViolation, but with the strings "b" and "a". Although
\* "b" appears first in this module, TLC enumerates "a" first and reports
\* InvA. The names a and b were already in TLC's intern table before this
\* module's strings, from standard modules TLC always loads (Naturals,
\* Sequences, TLC, ... are parsed even without EXTENDS; see the SANY lines of
\* the transcript), and "a" comes before "b" there. So TLC's string order is not
\* the order of first appearance in the user's module alone.
VARIABLE y
Init == y = "init"
First == "b"
Next == \E v \in {"a", First} : y' = v
InvA == y # "a"
InvB == y # "b"
====
