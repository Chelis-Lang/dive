---- MODULE ErHoConstBagOfAll ----
\* TLC cannot apply an operator with a constant body passed to BagOfAll
\* (error 2266: "should be an operator"); the core refuses the call.
EXTENDS Naturals, Bags
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Inv == BagCardinality(BagOfAll(LAMBDA y : 0, SetToBag({x, 5}))) = 2
====
