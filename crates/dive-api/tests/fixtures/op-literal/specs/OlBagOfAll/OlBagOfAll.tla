---- MODULE OlBagOfAll ----
\* BagOfAll checks its operator argument itself (2266).
EXTENDS Naturals, Sequences, SequencesExt, Bags, FiniteSetsExt
CONSTANT N
ASSUME BagCardinality(BagOfAll(LAMBDA x : 0, SetToBag({1, 2}))) = 2
====
