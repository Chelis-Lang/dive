---- MODULE PilotWeakStuckRequirements ----
EXTENDS Integers, Sequences, FiniteSets, TLC
\* Generated from finite requirement model sha256:fceb584c5907d1f2a03cef2c4fa0f7aeb3f6bac35096506677618f2b8c6281a2
VARIABLES state_73746174653a76616c7565, mon_524553504f4e5345_pending, mon_524553504f4e5345_ack
vars == <<state_73746174653a76616c7565, mon_524553504f4e5345_pending, mon_524553504f4e5345_ack>>
Req_4d4f5645 == ((state_73746174653a76616c7565 = 0 /\ state_73746174653a76616c7565' = 0) \/ (state_73746174653a76616c7565 = 1 /\ state_73746174653a76616c7565' = 0) \/ (state_73746174653a76616c7565 = 2 /\ state_73746174653a76616c7565' = 0))
Always_Req_4d4f5645 == [][Req_4d4f5645]_vars
Trigger_Req_524553504f4e5345 == ((state_73746174653a76616c7565 = 0))
Response_Req_524553504f4e5345 == ((state_73746174653a76616c7565 = 1))
Req_524553504f4e5345 == ([]((mon_524553504f4e5345_pending /\ ~mon_524553504f4e5345_ack) => <>(mon_524553504f4e5345_ack))) /\ ([]((mon_524553504f4e5345_pending /\ mon_524553504f4e5345_ack) => <>(~mon_524553504f4e5345_ack)))
RequirementMonitorInit == (mon_524553504f4e5345_pending = Trigger_Req_524553504f4e5345 /\ mon_524553504f4e5345_ack = FALSE)
RequirementMonitorNext == (mon_524553504f4e5345_pending' = (((state_73746174653a76616c7565' = 0)) \/ (mon_524553504f4e5345_pending /\ ~(((state_73746174653a76616c7565' = 1))))) /\ mon_524553504f4e5345_ack' = IF mon_524553504f4e5345_pending /\ (((state_73746174653a76616c7565' = 1))) THEN ~mon_524553504f4e5345_ack ELSE mon_524553504f4e5345_ack)
RequirementInit == (((state_73746174653a76616c7565 = 0))) /\ RequirementMonitorInit
RequirementNext == (Req_4d4f5645 /\ ((state_73746174653a76616c7565 = 0 /\ state_73746174653a76616c7565' = 0) \/ (state_73746174653a76616c7565 = 0 /\ state_73746174653a76616c7565' = 1) \/ (state_73746174653a76616c7565 = 0 /\ state_73746174653a76616c7565' = 2) \/ (state_73746174653a76616c7565 = 1 /\ state_73746174653a76616c7565' = 0) \/ (state_73746174653a76616c7565 = 1 /\ state_73746174653a76616c7565' = 1) \/ (state_73746174653a76616c7565 = 1 /\ state_73746174653a76616c7565' = 2) \/ (state_73746174653a76616c7565 = 2 /\ state_73746174653a76616c7565' = 0) \/ (state_73746174653a76616c7565 = 2 /\ state_73746174653a76616c7565' = 1) \/ (state_73746174653a76616c7565 = 2 /\ state_73746174653a76616c7565' = 2))) /\ RequirementMonitorNext
====
