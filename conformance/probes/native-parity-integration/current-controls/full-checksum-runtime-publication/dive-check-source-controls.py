from pathlib import Path
import subprocess,sys
names=['test-validation-import.py','test-frontend-campaign.py','test-parity-ledger.py','check-parity-ledger.py','check-tlc-planning.py','check-tlc-tableau.py','check-temporal-action-order.py','check-phase-l.py','check-phase-l-graph.py','check-phase-execution.py','check-state-if-planning.py','check-predecessor-spill.py','check-native-checkpoint.py','check-native-parity-integration.py']
names += [name for name in ['check-tlc-final.py','check-temporal-forwarding.py','check-tlc-final-safety.py','check-tlc-state-if.py','check-tlc-temporal-if.py','check-tlc-temporal-if-final.py','check-tlc-final-scopes.py'] if Path('scripts', name).exists()]
for name in names:
 print('RUN',name,flush=True);r=subprocess.run([sys.executable,'-I','scripts/'+name]);print('RESULT',r.returncode,flush=True)
 if r.returncode:raise SystemExit(r.returncode)
