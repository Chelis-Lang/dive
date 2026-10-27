---- MODULE CmSubSeqs ----
\* SequencesExt's AllSubSeqs, LongestCommonPrefix, and ReplaceFirstSubSeq
\* and ReplaceAllSubSeqs on strings, in crates/dive-core/src/stdlib/community.rs.
EXTENDS Naturals, Sequences, SequencesExt
VARIABLE s, w
Init == s = <<>> /\ w = "ab"
Next == \/ Len(s) < 3 /\ \E x \in 1..2 : s' = Append(s, x) /\ UNCHANGED w
        \/ \E q \in AllSubSeqs(s) : Len(q) = 1 /\ s' = q /\ w' = ReplaceAllSubSeqs("x", "a", w)
        \/ Len(w) < 6 /\ w' = ReplaceFirstSubSeq("ab", "b", w) /\ UNCHANGED s
Inv == /\ s \in AllSubSeqs(s) /\ <<>> \in AllSubSeqs(s)
       /\ AllSubSeqs(<<1, 2, 3>>) = {<<>>, <<1>>, <<2>>, <<3>>, <<1, 2>>, <<1, 3>>, <<2, 3>>, <<1, 2, 3>>}
       /\ AllSubSeqs(<<1, 1>>) = {<<>>, <<1>>, <<1, 1>>}
       /\ AllSubSeqs(<<>>) = {<<>>}
       /\ (CHOOSE q \in AllSubSeqs(<<3, 1>>) : TRUE) = <<>>
       /\ LongestCommonPrefix({s}) = s
       /\ LongestCommonPrefix({s, s \o <<7>>}) = s
       /\ LongestCommonPrefix({<<1, 2, 3>>, <<1, 2, 4>>, <<1, 3>>}) = <<1>>
       /\ LongestCommonPrefix({<<2>>, <<1>>}) = <<>>
       /\ LongestCommonPrefix({"abc", "abd"}) = "ab"
       /\ LongestCommonPrefix({"abc"}) = "abc"
       /\ LongestCommonPrefix({"xy", "zw"}) = ""
       /\ ReplaceFirstSubSeq("X", "b", "abab") = "aXab"
       /\ ReplaceFirstSubSeq("X", "", "ab") = "Xab"
       /\ ReplaceFirstSubSeq("X", "q", "ab") = "ab"
       /\ ReplaceAllSubSeqs("X", "b", "abab") = "aXaX"
       /\ ReplaceAllSubSeqs("-", "", "abc") = "-a-b-c"
       /\ ReplaceAllSubSeqs("yy", "aa", "aaa") = "yya"
       /\ Len(w) >= 2
====
