---- MODULE PureInstance ----
EXTENDS Integers, Sequences
VARIABLE inbox
Init == inbox = <<[type |-> "pl"]>>
Next == inbox' = CASE Len(inbox) = 1 /\ inbox[1].type = "pl" -> <<[type |-> "tok"]>>
                [] Len(inbox) = 1 -> <<[type |-> "pl"], [type |-> "pl"]>>
                [] OTHER -> inbox
Abs == INSTANCE PayloadAbs WITH pending <- Len(SelectSeq(inbox, LAMBDA msg : msg.type = "pl"))
Prop == Abs!Spec
====
