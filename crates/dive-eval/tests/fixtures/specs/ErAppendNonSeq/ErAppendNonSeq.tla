---- MODULE ErAppendNonSeq ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME Append(1, 2) = <<>>
====
