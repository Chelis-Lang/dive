---- MODULE Procedure_P_parallel ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0,b=1)

begin
P: a:=b || b:=a; R: x:=a; return;
end procedure;
begin
L: call Put(2,3); E: skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "cd5a596a" /\ chksum(tla) = "51bb4b0c")
VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 0
        /\ b = 1
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ /\ a' = b
        /\ b' = a
     /\ pc' = "R"
     /\ UNCHANGED << x, stack >>

R == /\ pc = "R"
     /\ x' = a
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)

Put == P \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ b' = 3
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "E",
                         a         |->  a,
                         b         |->  b ] >>
                     \o stack
     /\ pc' = "P"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, b >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
