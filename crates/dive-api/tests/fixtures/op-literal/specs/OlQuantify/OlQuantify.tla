---- MODULE OlQuantify ----
\* Quantify takes an OpValue in its second position (2154).
EXTENDS Naturals, Sequences, SequencesExt, Bags, FiniteSetsExt
CONSTANT N
ASSUME Quantify({1, 2}, LAMBDA x : 0) = 0
====
