---- MODULE RecOpCalls ----
EXTENDS Naturals, Sequences, TLC
VARIABLE x
Init == x=0
Next == x'=IF x<2 THEN x+1 ELSE x
MapSeq(seq, Op(_)) == LET F[i \in 0..Len(seq)] == IF i=0 THEN << >>
                         ELSE Append(F[i-1], Op(seq[i]))
                       IN F[Len(seq)]
Through(seq, Op(_)) == MapSeq(seq, Op)
FilterSeq(seq, Test(_)) == LET F[i \in 0..Len(seq)] == IF i=0 THEN << >>
                            ELSE SelectSeq(Append(F[i-1],seq[i]),Test)
                          IN F[Len(seq)]
Inc(v) == v+1
Local(a) == LET Map(q, Op(_)) == LET F[i \in 0..Len(q)] == IF i=0 THEN << >>
                                  ELSE Append(F[i-1], Op(q[i]))
                                IN F[Len(q)]
            IN Map(<<1,2>>, LAMBDA v: v+a)
Good == /\ MapSeq(<<x,x+1>>,Inc)= <<x+1,x+2>>
        /\ MapSeq(<<x>>,LAMBDA v: v+2)= <<x+2>>
        /\ Through(<<x>>,Inc)= <<x+1>>
        /\ FilterSeq(<<x,0>>,LAMBDA v: v>0)=IF x=0 THEN << >> ELSE <<x>>
        /\ Local(x)= <<x+1,x+2>>
        /\ Local(x+3)= <<x+4,x+5>>
        /\ MapSeq(<< >>,LAMBDA v: 1 \div 0)= << >>
        /\ MapSeq(<<x>>,LAMBDA v: IF v=x THEN v ELSE 1 \div 0)= <<x>>
Wrong == Good /\ MapSeq(<<x>>,Inc)= <<1>>
Far(a) == MapSeq(<<1>>,LAMBDA v: v+a)
OutsideCapture == Far(x)= <<x+1>>
PrimeCapture == (Local(x))'= <<x'+1,x'+2>>
PrimeCaptureCorrect == [][PrimeCapture]_x
PrimeCaptureWrong == [][(Local(x))'= <<x+1,x+2>>]_x
Effect == MapSeq(<<x>>,LAMBDA v: Print("map",v))= <<x>>
CapturedEffect == Local(Print("map",x))= <<x+1,x+2>>
RECURSIVE Hidden(_)
Hidden(v) == IF v=0 THEN Print("hidden",x) ELSE Hidden(v-1)
CapturedCall == Local(Hidden(x))= <<x+1,x+2>>
RECURSIVE Sum(_)
Sum(v) == IF v=0 THEN 0 ELSE v+Sum(v-1)
RecursiveCallback == MapSeq(<<x>>,Sum)= <<Sum(x)>>
UseRecursive(Op(_)) == LET RECURSIVE R(_)
                          R(n) == IF n=0 THEN x ELSE Op(n)+R(n-1)
                      IN R(2)
RecursiveBoundary == UseRecursive(Inc)=x+5
EnabledCall(Op(_)) == LET F[n \in 0..1] == IF n=0 THEN 0
                         ELSE IF ENABLED Op(n) THEN F[n-1]+1 ELSE 0
                      IN F[1]
Step(v) == x'=v
ActionBoundary == EnabledCall(Step)=1
Both(Op(_)) == LET F[n \in 0..2] == IF n=0 THEN x ELSE Op(n)+F[n-1]
               IN F[2]=x+5 /\ (F[2])'=x'+5
BadBoth(Op(_)) == LET F[n \in 0..2] == IF n=0 THEN x ELSE Op(n)+F[n-1]
                  IN F[2]=x+5 /\ (F[2])'=x+5
PrimeCorrect == [][Both(Inc)]_x
PrimeWrong == [][BadBoth(Inc)]_x
====
