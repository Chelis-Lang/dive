---- MODULE Norm_P_while_label_root ----
EXTENDS Naturals, TLC
(* --algorithm A variables x=0; begin while x<2 do x:=x+1; end while; skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "beff5404" /\ chksum(tla) = "1b7fae54")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Step1"

Step1 == /\ pc = "Step1"
         /\ IF x<2
               THEN /\ x' = x+1
                    /\ pc' = "Step1"
               ELSE /\ TRUE
                    /\ pc' = "Done"
                    /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Step1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
