---- MODULE Schedule ----
EXTENDS EnvironmentController
Victim == N
HasBox == \E b \in inTransit : b.content.to = Victim
Remaining == {i \in Proc : ~failed[i] /\ moved[i] = "NO"}
First == CHOOSE i \in Remaining : \A j \in Remaining : i <= j
Scheduled ==
  /\ Next
  /\ IF ~failed[Victim] /\ HasBox
        THEN failed'[Victim]
        ELSE IF Remaining = {}
          THEN EnvTick
          ELSE /\ ~failed'[First]
               /\ moved'[First] \in {"PREDICT","RECEIVE","SEND"}
               /\ (moved'[First] = "RECEIVE" => (\A b \in inTransit' : b.content.to # First))
====
