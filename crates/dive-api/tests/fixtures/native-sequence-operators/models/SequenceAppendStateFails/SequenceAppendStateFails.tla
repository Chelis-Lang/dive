---- MODULE SequenceAppendStateFails ----
EXTENDS Sequences
VARIABLE x
Init==x = <<TRUE>>
Next==UNCHANGED x
Use(F(_,_))==[]<>(~(F(x, TRUE)= <<TRUE, TRUE>>))
Live==Use(Append)
====
