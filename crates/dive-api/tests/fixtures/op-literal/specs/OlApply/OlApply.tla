---- MODULE OlApply ----
\* Applied by a TLA+ operator, the value is the result.
EXTENDS Naturals, Sequences, SequencesExt, Bags, FiniteSetsExt
CONSTANT N
Apply(F(_), v) == F(v)
ASSUME Apply(LAMBDA x : 0, 1) = 0
====
