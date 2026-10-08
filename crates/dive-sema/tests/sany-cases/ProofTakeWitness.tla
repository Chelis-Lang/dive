---- MODULE ProofTakeWitness ----
THEOREM \A x \in {1} : \E y \in {1} : y = x
<1>1. TAKE x \in {1}
<1>2. WITNESS 1 \in {1}
<1>3. QED
  OBVIOUS
====
