---- MODULE Forms_P_symbolic_local ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 LOCAL a (+) b == a+b
 end define;
begin L:y:=1 (+) 2; E:skip;
end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "c1a05fb7" /\ chksum(tla) = "ffbaf738")
VARIABLES pc, x, y

(* define statement *)
LOCAL a (+) b == a+b


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ y' = 1 (+) 2
     /\ pc' = "E"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
