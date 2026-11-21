---- MODULE TunableSetPolicies ----
EXTENDS TunableSetHistory

\* Policies 1, 2, 3 denote normal, priority and LWW for each operation kind.
SameItem(a,b) == a.elem = b.elem
Earlier(a,b) == a \in b.observed
Concurrent(a,b) == a /= b /\ ~Earlier(a,b) /\ ~Earlier(b,a)
ApplyPolicy(owner,a,b) ==
    /\ SameItem(a,b)
    /\ CASE owner.policy = 3 -> a.policy = 3 /\ (b.policy /= 3 \/ a.stamp > b.stamp)
         [] owner.kind = "add" /\ owner.policy = 1 ->
              a.kind = "add" /\ b.kind = "remove" /\ b.policy = 1
         [] owner.kind = "remove" /\ owner.policy = 2 ->
              a.kind = "remove" /\ b.kind = "add" /\ b.policy = 1
         [] owner.kind = "add" /\ owner.policy = 2 ->
              a.kind = "add" /\ b.kind = "remove" /\ b.policy /= 3
         [] OTHER -> FALSE
Maximal(S) == {b \in S : ~(\E a \in S : SameItem(a,b) /\ Earlier(b,a))}
Obsolete(S) == {b \in S : \E a \in S :
                   Concurrent(a,b) /\ (ApplyPolicy(b,a,b) \/ ApplyPolicy(a,a,b))}
Active(S) == LET M == Maximal(S) IN M \ Obsolete(M)
Has(S,e,k) == \E x \in S : x.elem=e /\ x.kind=k
Lookup(S,e) == Has(S,e,"add")
Items(S) == {e \in Elements : Lookup(S,e)}
AllFinished == \A r \in Replicas : pc[r] = "Done"
ObservableConvergence == AllFinished =>
    \A a,b \in Replicas :
       /\ Items(Active(known[a])) = Items(Active(known[b]))
       /\ \A e \in Elements : Lookup(Active(known[a]),e) = Lookup(Active(known[b]),e)
NoSimultaneousAddRemove == \A r \in Replicas, e \in Elements :
    ~(Has(Active(known[r]),e,"add") /\ Has(Active(known[r]),e,"remove"))
AtLeastOneOp == \A r \in Replicas : \A x \in known[r] : \E y \in Active(known[r]) : SameItem(x,y)

SameOtherElements(x,A,B) ==
    /\ Items(A) \ {x.elem} = Items(B) \ {x.elem}
    /\ {a \in A : ~SameItem(a,x)} = {b \in B : ~SameItem(b,x)}
PSE(x,M,A,B) ==
   /\ SameOtherElements(x,A,B)
   /\ \/ /\ x \notin M /\ \E y \in A : SameItem(x,y) /\ Earlier(x,y)
         /\ Lookup(A,x.elem) = Lookup(B,x.elem)
      \/ /\ x \in M
         /\ ~(\E y \in M : SameItem(x,y) /\ (Earlier(x,y) \/ Earlier(y,x)))
         /\ \/ x \notin B
            \/ (x.kind = "remove" /\ ~Lookup(B,x.elem))
            \/ (x.kind = "add" /\ Lookup(B,x.elem))
PPE(x,A,B) ==
    \/ ((x.kind = "add" /\ Has(A,x.elem,"add")) => (Lookup(A,x.elem) /\ Lookup(B,x.elem)))
    \/ ((x.kind = "remove" /\ Has(A,x.elem,"remove")) => (~Lookup(A,x.elem) /\ ~Lookup(B,x.elem)))
PCS(x,B) == (~Has(B,x.elem,"remove") /\ Lookup(B,x.elem))
          \/ (~Has(B,x.elem,"add") /\ ~Lookup(B,x.elem))
ReplayStep(S,x) ==
    LET T == S \cup {x}
        A == Active(S)
        B == Active(T)
    IN SameOtherElements(x,A,B) /\ PSE(x,Maximal(T),A,B) /\ PPE(x,A,B) /\ PCS(x,B)
CorrectnessPrinciples == AllFinished =>
    \A r \in Replicas :
       \A i \in 1..Len(history[r]) :
          ReplayStep({history[r][j] : j \in 1..(i-1)},history[r][i])
====
