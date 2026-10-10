---- MODULE Multi_P_infinite_one ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=0; begin L: while TRUE do x:=(x+1)%2; end while; end process;  process Q =2 variable y=0; begin M: y:=2; F: g:=y; end process; end algorithm; *)
====
