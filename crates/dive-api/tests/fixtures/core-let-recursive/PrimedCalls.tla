---- MODULE PrimedCalls ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x=0
Next == x' \in {1,2}
Good == LET f[i \in 0..x] == IF i=0 THEN x ELSE f[i-1]+1
        IN f[x]=2*x /\ (f[x])'=2*x'
Reverse == LET f[i \in 0..x] == IF i=0 THEN x ELSE f[i-1]+1
           IN (f[x])'=2*x' /\ f[x]=2*x
OnlyOne == LET f[i \in 0..x] == IF i=0 THEN x ELSE f[i-1]+1
           IN f[x]=2*x /\ (f[x])'=2
Bad == LET f[i \in 0..x] == IF i=0 THEN x ELSE f[i-1]+1
       IN f[x]=2*x /\ (f[x])'=2*x
Correct == [][Good]_x
Wrong == [][Bad]_x
Value == LET f[i \in 0..1] == IF i=0 THEN x ELSE f[i-1]+1
         IN f[1]
ValueStep == Value=x+1 /\ Value'=x'+1
ValueCorrect == [][ValueStep]_x
Unused == LET bad == 1 \div 0
              f[i \in 0..2] == IF i=0 THEN x
                              ELSE IF i=1 THEN f[i-1]+1 ELSE 1 \div 0
          IN (f[1])'=x'+1
Read(a) == LET f[i \in 0..1] == IF i=0 THEN a ELSE f[i-1]+1
           IN (f[1])'=x'+1
Captured == Read(x)
Mixed == LET f[i \in 0..1] == IF i=0 THEN x ELSE (f[i-1])'
         IN f[1]=x'
Effect == LET f[i \in 0..1] == IF i=0 THEN Print("value",x) ELSE f[i-1]+1
          IN (f[1])'=x'+1
EscapePrime == LET f[i \in 0..1] == IF i=0 THEN x ELSE f[i-1]+1
               IN (DOMAIN f)'={0,1}
====
