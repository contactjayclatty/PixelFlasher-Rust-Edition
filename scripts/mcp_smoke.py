"""Exercise the packaged stdio boundary in isolated data; never use device transport."""
from pathlib import Path
import json
import os
import subprocess
import sys
import tempfile

exe = Path(sys.argv[1] if len(sys.argv)>1 else 'dist/pixelflasher-mcp.exe').resolve()
with tempfile.TemporaryDirectory(prefix='pixelflasher-mcp-') as directory:
    data=Path(directory)
    env=dict(os.environ, PIXELFLASHER_DATA_DIR=str(data))
    def run(requests):
        result=subprocess.run([str(exe)],input=''.join(json.dumps(r)+'\n' for r in requests),text=True,capture_output=True,env=env,timeout=20,check=True)
        return [json.loads(line) for line in result.stdout.splitlines()]
    def request(ident,name,args=None):
        return {'jsonrpc':'2.0','id':ident,'method':'tools/call','params':{'name':name,'arguments':args or {}}}
    disabled=run([request(1,'inspect')])[0]
    assert disabled['result']['isError'] and 'MCP_DISABLED' in disabled['result']['content'][0]['text']
    (data/'settings.json').write_text(json.dumps({'mcp_enabled':True,'devices':[{'serial':'MCP_FIXTURE_1234','label':'SECRET_CREDENTIAL MCP_FIXTURE_1234 C:/private','enabled':True}]}),encoding='utf-8')
    rows=run([{'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'fixture','version':'1'}}},{'jsonrpc':'2.0','method':'notifications/initialized'},{'jsonrpc':'2.0','id':2,'method':'tools/list'},request(3,'inspect')])
    assert len(rows)==3
    assert rows[0]['result']['serverInfo']['name']=='pixelflasher-mcp'
    names={t['name'] for t in rows[1]['result']['tools']}
    assert names=={'inspect','capabilities','prepare','status'}
    inspected=rows[2]['result']['structuredContent']['result']
    assert not any(secret in json.dumps(inspected) for secret in ['MCP_FIXTURE_1234','SECRET_CREDENTIAL','C:/private'])
    device=inspected['devices'][0]['id']
    intent={'kind':'OpenManager','device_id':device,'provider':'KernelSuNext'}
    attacks=[request(4,'execute'),request(5,'shell',{'command':'whoami'}),request(6,'prepare',{'intent':dict(intent,secret='DO_NOT_ECHO')}),request(7,'prepare',{'intent':dict(intent,device_id='overridden')})]
    assert all(r['result']['isError'] for r in run(attacks))
    queued=run([request(8,'prepare',{'intent':intent})])[0]['result']
    assert not queued['isError'] and queued['structuredContent']['result']['execution_allowed'] is False
    import sqlite3
    with sqlite3.connect(data/'library.db') as db:
        assert db.execute('SELECT count(*) FROM review_queue').fetchone()[0]==1
        assert db.execute('SELECT count(*) FROM journal').fetchone()[0]==0
    db.close()
    report={'executable':exe.name,'disabled_default':True,'stdio_handshake':True,'tools':sorted(names),'redaction':True,'forbidden_tools_and_overrides_rejected':True,'prepare_queued_without_execution':True,'device_transport_calls':0}
print(json.dumps(report,indent=2))
