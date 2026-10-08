---- MODULE UsesCommunityModules ----
EXTENDS Naturals, SequencesExt, FiniteSetsExt
VARIABLE x
Init == x = 0
Next == x' = (x + Max({1, 2, SumSet({1, 2})})) % 7
Inv == ToSet(<<x>>) \subseteq 0..6
====
