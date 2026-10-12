---- MODULE Procedure_P_loop ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: while a<3 do a:=a+1; end while; R: x:=a; return;
end procedure;
begin
L: call Put(0); E: skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "cdc74064" /\ chksum(tla) = "ab312b14")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ IF a<3
           THEN /\ a' = a+1
                /\ pc' = "P"
           ELSE /\ pc' = "R"
                /\ a' = a
     /\ UNCHANGED << x, stack >>

R == /\ pc = "R"
     /\ x' = a
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)

Put == P \/ R

L == /\ pc = "L"
     /\ /\ a' = 0
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "E",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "P"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
