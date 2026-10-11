---- MODULE MacroLayout_P_multiline_arg ----
EXTENDS Integers,TLC
(* --algorithm A variable x=0; macro Put(v,a) begin v:=a; end macro; begin L:Put(x,1+
 2); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "550dfca8" /\ chksum(tla) = "7c70405c")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' =                                                                                   1+
             2
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
