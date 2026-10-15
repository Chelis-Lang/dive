---- MODULE CompositionFinal ----
VARIABLE x
Init == x = FALSE
First == x' = TRUE
Last == x' = x
Next == First \cdot Last
Fair == x' = TRUE
Spec == Init /\ [][Next]_x /\ WF_x(Fair)
Hold == <>x
Fail == []<>~x
====
