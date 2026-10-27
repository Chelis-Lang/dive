---- MODULE CmOps ----
\* CommunityModules value operators in crates/dive-core/src/stdlib/community.rs:
\* SequencesExt's Contains, RemoveFirst, Suffixes, SetToSeq and IsPrefix,
\* Functions' IsInjective and Bitwise's &, |, ^^, Not and shiftR.
EXTENDS Naturals, Sequences, SequencesExt, Functions, Bitwise
VARIABLE s
Init == s \in {<<>>, <<1>>, <<2, 1>>}
Next == \/ Len(s) < 3 /\ \E x \in 1..3 : ~Contains(s, x) /\ s' = Append(s, x)
        \/ \E x \in ToSet(s) : s' = RemoveFirst(s, x)
Inv == /\ IsInjective(s)
       /\ s \in Suffixes(s)
       /\ \A t \in Suffixes(s) : Len(t) <= Len(s)
       /\ IsPrefix(<<>>, s) /\ (s # <<>> => ~IsPrefix(s \o <<9>>, s))
       /\ IsPrefix("ab", "abc") /\ ~IsPrefix("b", "abc")
       /\ SetToSeq({3, 1, 2}) = <<1, 2, 3>>
       /\ Len(SetToSeq(ToSet(s))) = Len(s)
       /\ ~IsInjective(<<1, 1>>) /\ IsInjective([a |-> 1, b |-> 2])
       /\ RemoveFirst(<<1, 2, 1>>, 1) = <<2, 1>>
       /\ (Len(s) & 2) <= 2 /\ (5 | 2) = 7 /\ (6 ^^ 3) = 5
       /\ Not(5) = 2 /\ shiftR(16, 2) = 4
====
