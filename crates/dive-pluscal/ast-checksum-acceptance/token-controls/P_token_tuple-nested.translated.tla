---- MODULE P_token_tuple_nested ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x = <<<<0>>>>; begin L:x:=<<1>>; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "afd19d29" /\ chksum(tla) = "62016375")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = <<<<0>>>>
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = <<1>>
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
