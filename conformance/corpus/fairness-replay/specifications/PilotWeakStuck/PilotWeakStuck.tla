---- MODULE PilotWeakStuck ----
EXTENDS PilotWeakStuckRequirements
Init == RequirementInit
Next == RequirementNext
Spec == Init /\ [][Next]_vars /\ WF_vars(Next)
====
