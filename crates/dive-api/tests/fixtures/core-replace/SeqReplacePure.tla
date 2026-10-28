---- MODULE SeqReplacePure ----
EXTENDS Naturals, Sequences, SequencesExt
VARIABLE x
Init == x = 0
Next == x' = x
Inv == /\ x = 0
       /\ ReplaceFirstSubSeq(<<9>>, <<>>, <<1,2>>) = <<9,1,2>>
       /\ ReplaceAllSubSeqs(<<9>>, <<>>, <<1,2>>) = <<9,1,9,2>>
       /\ ReplaceAllSubSeqs(<<4>>, <<>>, <<>>) = <<4>>
       /\ ReplaceFirstSubSeq(<<9>>, <<1,1>>, <<1,1,1>>) = <<9,1>>
       /\ ReplaceAllSubSeqs(<<9>>, <<1,1>>, <<1,1,1>>) = <<9,1>>
       /\ ReplaceFirstSubSeq(<<9>>, <<1>>, <<1,2,1>>) = <<9,2,1>>
       /\ ReplaceAllSubSeqs(<<9>>, <<1>>, <<1,2,1>>) = <<9,2,9>>
       /\ ReplaceAllSubSeqs(<<>>, <<1>>, <<1,2,1>>) = <<2>>
       /\ ReplaceAllSubSeqs(<<1>>, <<1>>, <<1,2,1>>) = <<1,2,1>>
       /\ ReplaceAllSubSeqs(<<9,9>>, <<1,2>>, <<1,2>>) = <<9,9>>
       /\ ReplaceAllSubSeqs(<<9>>, <<1,2,3>>, <<1,2>>) = <<1,2>>
       /\ ReplaceAllSubSeqs(<<FALSE>>, <<TRUE>>, <<TRUE,FALSE,TRUE>>) = <<FALSE,FALSE,FALSE>>
       /\ ReplaceFirstSubSeq(<<"z">>, <<"a">>, <<"a","b","a">>) = <<"z","b","a">>
       /\ ReplaceAllSubSeqs(<<"z">>, <<"a">>, <<"a","b","a">>) = <<"z","b","z">>
====
