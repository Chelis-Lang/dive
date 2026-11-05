---- MODULE LetRecCalls ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x=0
Next == x'=IF x<2 THEN x+1 ELSE x
RECURSIVE Outer(_)
Outer(a) == LET f[n \in 0..2] == IF n=0 THEN a ELSE f[n-1]+a
            IN f[2] + IF a=0 THEN 0 ELSE Outer(a-1)
Shadow(a) == LET v == a+1
                Inner(b) == LET f[n \in 0..2] == IF n=0 THEN a+b ELSE f[n-1]+a+b
                            IN f[2]
            IN Inner(v)
Lazy(a, unused) == LET bad == 1 \div 0
                      f[n \in 0..2] == IF n=0 THEN a ELSE f[n-1]+a
                  IN f[2]
State == LET f[n \in 0..(x+2)] == IF n=0 THEN x ELSE f[n-1]+1
         IN f[2]
A == INSTANCE RecCapture WITH K <- x
B == INSTANCE RecCapture WITH K <- x+2
Inv == /\ Outer(x)=(3*x*(x+1)) \div 2
       /\ Outer(x+1)=(3*(x+1)*(x+2)) \div 2
       /\ Shadow(x)=3*(2*x+1)
       /\ Lazy(x,1 \div 0)=3*x
       /\ State=x+2
       /\ A!Read(1)=3*(x+1)
       /\ B!Read(1)=3*(x+3)
Wrong == Inv /\ Outer(x)=0
Escape == LET f[n \in 0..2] == IF n=0 THEN x ELSE f[n-1]+1
          IN DOMAIN f={0,1,2}
Prime == LET f[n \in 0..2] == IF n=0 THEN x ELSE f[n-1]+1
         IN (f[1])'=x'+1
Inc(n) == n+1
Use(Op(_)) == LET f[n \in 0..2] == IF n=0 THEN x ELSE Op(n)+f[n-1]
              IN f[2]
OperatorCapture == Use(Inc)=x+5
Nested == LET f[n \in 0..2] == IF n=0 THEN x
                              ELSE LET g[m \in 0..2] == IF m=0 THEN f[0] ELSE g[m-1]
                                   IN g[n]
          IN f[1]=x
====
