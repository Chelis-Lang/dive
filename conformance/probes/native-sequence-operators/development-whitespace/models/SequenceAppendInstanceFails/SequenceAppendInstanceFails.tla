---- MODULE SequenceAppendInstanceFails ----
EXTENDS Sequences
VARIABLE x
S==INSTANCE Sequences
Init==x=<<TRUE>>
Next==UNCHANGED x
Use(F(_,_))==[]<>(~(F(x, TRUE)=<<TRUE, TRUE>>))
Live==Use(S!Append)
====
