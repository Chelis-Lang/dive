---- MODULE NativeAdmission ----
EXTENDS Naturals
T == INSTANCE TLC
CONSTANTS A,B
VARIABLE x
S == INSTANCE Sequences
Positive(v) == v > 0
Init == S!SelectSeq(<<1,2>>,Positive)=<<1,2>> /\ x=0
Next == x<3 /\ x'=x+1
View == x
Perms == T!Permutations({A,B})
====
