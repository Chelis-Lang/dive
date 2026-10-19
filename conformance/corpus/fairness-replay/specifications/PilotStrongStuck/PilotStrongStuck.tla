---- MODULE PilotStrongStuck ----
EXTENDS PilotStrongStuckRequirements
Init == RequirementInit
Next == RequirementNext
Spec == Init /\ [][Next]_vars /\ SF_vars(Next)
====
