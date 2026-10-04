#!/usr/bin/env python3
"""Exercise an installed gateway or container with local simulated upstreams.

Requires requests and PyYAML. TLS certificates and a fresh fixture directory are
supplied by the caller; no vendor API, production credential, or user database
is used. See docs/gateway/release-0.8.0-verification.md for commands.
"""
import argparse, base64, hashlib, http.server, json, os, pathlib, socket, sqlite3, ssl, struct, subprocess, threading, time
from urllib.parse import urlsplit
import secrets
import requests, yaml


def read_frame(stream):
    header = stream.read(2)
    if len(header) != 2: return None, None
    opcode, size = header[0] & 15, header[1] & 127
    if size == 126: size = struct.unpack('!H', stream.read(2))[0]
    elif size == 127: size = struct.unpack('!Q', stream.read(8))[0]
    mask = stream.read(4) if header[1] & 128 else None
    body = stream.read(size)
    if mask: body = bytes(v ^ mask[i % 4] for i,v in enumerate(body))
    return opcode, body

def send_frame(stream, body, opcode=1, masked=False):
    if isinstance(body, str): body = body.encode()
    n = len(body); flag = 128 if masked else 0
    head = bytes([128 | opcode, flag | (n if n < 126 else 126)])
    if n >= 126: head += struct.pack('!H', n)
    if masked:
        mask = os.urandom(4); head += mask
        body = bytes(v ^ mask[i % 4] for i,v in enumerate(body))
    stream.write(head + body); stream.flush()

class Mock(http.server.BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def log_message(self, *args): pass
    def reply(self, body, status=200, content_type='application/json'):
        body = body if isinstance(body, bytes) else json.dumps(body).encode()
        self.send_response(status); self.send_header('content-type', content_type)
        self.send_header('content-length', str(len(body))); self.send_header('connection', 'close')
        if status == 429: self.send_header('retry-after', '9')
        self.end_headers(); self.wfile.write(body)
    def do_GET(self):
        if '/realtime' not in self.path: return self.reply({'object':'list','data':[]})
        accept = base64.b64encode(hashlib.sha1((self.headers['sec-websocket-key']+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode()
        self.send_response(101); self.send_header('upgrade','websocket'); self.send_header('connection','Upgrade'); self.send_header('sec-websocket-accept',accept); self.end_headers()
        send_frame(self.wfile, json.dumps({'type':'session.created','session':{'id':'release-session','model':'gpt-realtime-mini'}}))
        while True:
            opcode, body = read_frame(self.rfile)
            if opcode is None: break
            if opcode == 8:
                send_frame(self.wfile, body, opcode=8); break
            if opcode == 9:
                send_frame(self.wfile, body, opcode=10); continue
            if opcode != 1: continue
            event = json.loads(body)
            if event['type'] == 'session.update':
                send_frame(self.wfile, json.dumps({'type':'session.updated','session':event['session']}))
            elif event['type'] == 'response.create':
                usage={'total_tokens':60,'input_tokens':40,'output_tokens':20,'input_token_details':{'text_tokens':20,'audio_tokens':20,'cached_tokens':15,'cached_tokens_details':{'text_tokens':10,'audio_tokens':5},'image_tokens':0},'output_token_details':{'text_tokens':5,'audio_tokens':15}}
                send_frame(self.wfile,json.dumps({'type':'response.created','response':{'id':'release-response'}}))
                send_frame(self.wfile,json.dumps({'type':'response.output_text.delta','response_id':'release-response','delta':'release hello'}))
                send_frame(self.wfile,json.dumps({'type':'response.done','response':{'id':'release-response','status':'completed','usage':usage}}))
    def do_POST(self):
        body=json.loads(self.rfile.read(int(self.headers['content-length'])))
        if body.get('mock_error'): return self.reply({'error':{'message':'release capacity unavailable'}},429)
        if self.path.endswith('/responses/input_tokens'): return self.reply({'object':'response.input_tokens','input_tokens':12})
        if self.path.endswith('/responses/compact'): return self.reply({'id':'compact-release','object':'response.compaction','output':[{'type':'compaction','id':'cmp-release','encrypted_content':'opaque=='}],'usage':{'input_tokens':12,'output_tokens':3,'total_tokens':15}})
        if self.path.endswith('/responses'):
            value={'id':'resp-release-'+secrets.token_hex(8),'object':'response','status':'completed','model':body['model'],'output':[{'type':'message','id':'msg-release','role':'assistant','content':[{'type':'output_text','text':'release hello'}]}],'usage':{'input_tokens':12,'output_tokens':3,'total_tokens':15},'release_native_marker':True}
            if body.get('tools'): value['output'].append({'type':'file_search_call','id':'search-release','status':'completed','queries':['release']})
            if body.get('stream'): return self.reply(('event: response.completed\ndata: '+json.dumps({'type':'response.completed','response':value})+'\n\n').encode(),content_type='text/event-stream')
            return self.reply(value)
        if self.path.endswith('/messages/count_tokens'): return self.reply({'input_tokens':12})
        if self.path.endswith('/messages'): return self.reply({'id':'msg-release','type':'message','role':'assistant','model':body['model'],'content':[{'type':'text','text':'release hello'}],'stop_reason':'end_turn','stop_sequence':None,'usage':{'input_tokens':12,'output_tokens':3},'release_native_marker':True})
        if ':generateContent' in self.path: return self.reply({'candidates':[{'content':{'role':'model','parts':[{'text':'release hello'}]},'finishReason':'STOP','index':0}],'usageMetadata':{'promptTokenCount':12,'candidatesTokenCount':3,'totalTokenCount':15},'modelVersion':'gemini-3.5-flash'})
        if self.path.endswith('/chat/completions'): return self.reply({'id':'chat-release','object':'chat.completion','created':1700000000,'model':body['model'],'choices':[{'index':0,'message':{'role':'assistant','content':'release hello'},'finish_reason':'stop'}],'usage':{'prompt_tokens':12,'completion_tokens':3,'total_tokens':15}})
        if self.path.endswith('/mcp'): return self.reply({'jsonrpc':'2.0','id':body['id'],'result':{'tools':[],'release_native_marker':True}})
        if self.path.endswith('/rpc'):
            task_id=('task-'+body['params']['message']['messageId']) if body['method']=='SendMessage' else body['params']['id']
            task={'id':task_id,'contextId':'context-'+task_id,'status':{'state':'TASK_STATE_COMPLETED'}}
            return self.reply({'jsonrpc':'2.0','id':body['id'],'result':{'task':task} if body['method']=='SendMessage' else task})
        self.reply({'error':{'message':'unknown mock endpoint '+self.path}},404)

def config(base, packaged, target, gateway, integrations=False):
    c=yaml.safe_load(pathlib.Path(packaged).read_text())
    c['server']['port']=urlsplit(gateway).port or 80; c['server']['cors']['enabled']=False
    c['providers']=[]
    for provider,models,version in [('openai',['gpt-4o-mini','gpt-realtime-mini'],'/v1'),('anthropic',['claude-haiku-4-5-20251001'],''),('gemini',['gemini-3.5-flash'],'')]:
        c['providers'].append({'name':provider,'provider_type':provider,'api_key':('sk-ant-' if provider=='anthropic' else 'sk-' if provider=='openai' else 'AIza')+'ReleaseSmokeOnlyNoVendorCalls1234567890','base_url':base+version,'models':models,'enabled':True,'timeout':10,'max_retries':0,'endpoint_access':'private_network'})
    c['auth'].update({'enable_jwt':True,'enable_api_key':False,'allow_anonymous':False,'jwt_secret':'ReleaseSmokeOnly_2026_NoProductionSecret!123'})
    c['storage']['database'].update({'enabled':True,'url':'sqlite://'+str(target/'smoke.sqlite')+'?mode=rwc','auto_migrate':True})
    c['pricing']['unpriced_model_policy']='reject'; c['pricing'].pop('unpriced_fallback_cost_per_1k_tokens',None)
    c['guardrails']={'enabled':False}
    if integrations:
        c['mcp_servers']={'smoke':{'name':'smoke','url':base+'/mcp','transport':'http','timeout_ms':10000}}
        c['a2a_agents']={'smoke':{'name':'smoke','url':base+'/rpc','timeout_ms':10000}}
    path=target/'config.json'; path.write_text(json.dumps(c)); return path

def smoke(gateway, integrations=False, database=None):
    s=requests.Session(); s.trust_env=False
    for _ in range(60):
        try:
            if s.get(gateway+'/health',timeout=1).status_code==200: break
        except requests.RequestException: pass
        time.sleep(.2)
    else: raise RuntimeError('Gateway did not start')
    username='release_smoke_'+secrets.token_hex(4)
    register=s.post(gateway+'/auth/register',json={'username':username,'email':username+'@example.invalid','password':'ReleaseSmokeOnly!Password123'},timeout=10)
    assert register.status_code==201,(register.status_code,register.text)
    if database:
        with sqlite3.connect(database) as db:
            db.execute("UPDATE users SET status='active', email_verified=1 WHERE username=?", (username,))
    login=s.post(gateway+'/auth/login',json={'username':username,'password':'ReleaseSmokeOnly!Password123'},timeout=10)
    assert login.status_code==200,(login.status_code,login.text)
    s.headers['authorization']='Bearer '+login.json()['data']['access_token']
    report=[]
    def post(name,path,body,headers=None,expected=200):
        r=s.post(gateway+path,json=body,headers=headers or {},timeout=45)
        assert r.status_code==expected,(name,r.status_code,r.text)
        report.append({'name':name,'status':r.status_code}); return r
    r=post('chat','/v1/chat/completions',{'model':'gpt-4o-mini','messages':[{'role':'user','content':'hello'}],'max_tokens':32}); assert r.json()['choices'][0]['message']['content']=='release hello'
    response={'model':'gpt-4o-mini','input':'hello','max_output_tokens':32}
    r=post('responses','/v1/responses',response); assert r.json()['release_native_marker']
    r=post('responses-sse','/v1/responses',{**response,'stream':True}); assert 'response.completed' in r.text, r.text
    r=post('file-search','/v1/responses',{**response,'tools':[{'type':'file_search','vector_store_ids':['vs-release']}],'max_tool_calls':1}); assert r.json()['output'][-1]['type']=='file_search_call'
    r=post('responses-error','/v1/responses',{**response,'mock_error':True},expected=429); assert r.headers['retry-after']=='9'
    r=post('messages','/v1/messages',{'model':'claude-haiku-4-5-20251001','messages':[{'role':'user','content':'hello'}],'max_tokens':32},headers={'anthropic-version':'2023-06-01'}); assert r.json()['release_native_marker']
    r=post('gemini','/v1beta/models/gemini-3.5-flash:generateContent',{'contents':[{'role':'user','parts':[{'text':'hello'}]}],'generationConfig':{'maxOutputTokens':32}}); assert r.json()['candidates'][0]['content']['parts'][0]['text']=='release hello'
    if integrations:
        r=post('mcp','/smoke/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/list','params':{'_meta':{'io.modelcontextprotocol/protocolVersion':'2026-07-28','io.modelcontextprotocol/clientCapabilities':{}}}},headers={'accept':'application/json, text/event-stream','MCP-Protocol-Version':'2026-07-28','Mcp-Method':'tools/list'}); assert r.json()['result']['release_native_marker']
        r=post('a2a-send','/a2a/smoke',{'jsonrpc':'2.0','id':1,'method':'SendMessage','params':{'message':{'role':'ROLE_USER','messageId':username,'parts':[{'text':'hello'}]}}},headers={'A2A-Version':'1.0'}); assert r.json()['result']['task']['id']=='task-'+username
        r=post('a2a-owned-task','/a2a/smoke',{'jsonrpc':'2.0','id':2,'method':'GetTask','params':{'id':'task-'+username}},headers={'A2A-Version':'1.0'}); assert r.json()['result']['id']=='task-'+username
    host,port=gateway.removeprefix('http://').split(':'); sock=socket.create_connection((host,int(port)),timeout=15); f=sock.makefile('rwb',buffering=0)
    key=base64.b64encode(os.urandom(16)).decode()
    f.write((f'GET /v1/realtime?model=gpt-realtime-mini HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: {key}\r\nAuthorization: '+s.headers['authorization']+'\r\n\r\n').encode())
    line=f.readline(); assert b'101' in line,line
    while f.readline()!=b'\r\n': pass
    initial=[json.loads(read_frame(f)[1]) for _ in range(2)]; assert initial[-1]['type']=='session.updated',initial
    send_frame(f,json.dumps({'type':'response.create','event_id':'release-create','response':{'max_output_tokens':32}}),masked=True)
    events=[]
    for _ in range(8):
        opcode,payload=read_frame(f); assert opcode==1,(opcode,payload)
        event=json.loads(payload); events.append(event)
        if event['type']=='response.done': break
    assert events[-1]['type']=='response.done',events
    assert events[-1]['response']['usage']['total_tokens']==60
    send_frame(f,struct.pack('!H',1000),opcode=8,masked=True); sock.close()
    report.append({'name':'realtime-ws','status':101,'completed_tokens':60})
    return report

if __name__=='__main__':
    p=argparse.ArgumentParser(); p.add_argument('mode',choices=['mock','local','remote-config','remote-smoke']); p.add_argument('--binary'); p.add_argument('--packaged'); p.add_argument('--directory',required=True); p.add_argument('--certificates'); p.add_argument('--base',default='https://127.0.0.1:18443'); p.add_argument('--gateway',default='http://127.0.0.1:18808'); a=p.parse_args()
    target=pathlib.Path(a.directory).resolve(); target.mkdir(parents=True,exist_ok=True)
    if a.mode in ('local','mock') and not a.certificates: p.error('--certificates is required for local/mock')
    certificates=pathlib.Path(a.certificates).resolve() if a.certificates else None
    if a.mode in ('local','remote-config') and not a.packaged: p.error('--packaged is required for configuration')
    if a.mode=='local' and not a.binary: p.error('--binary is required for local')
    if a.mode=='mock':
        httpd=http.server.ThreadingHTTPServer(('0.0.0.0',18443),Mock); ctx=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER); ctx.load_cert_chain(certificates/'mock-cert.pem',certificates/'mock-key.pem'); httpd.socket=ctx.wrap_socket(httpd.socket,server_side=True); httpd.serve_forever()
    elif a.mode=='remote-config': print(config(a.base,a.packaged,target,a.gateway,True))
    elif a.mode=='remote-smoke': print(json.dumps(smoke(a.gateway,True,database=target/'smoke.sqlite'),indent=2))
    else:
        if (target/'smoke.sqlite').exists(): raise FileExistsError('Use a fresh fixture directory; smoke.sqlite already exists')
        cfg=config(a.base,a.packaged,target,a.gateway)
        mocklog=(target/'mock.log').open('w')
        mock=subprocess.Popen(['python3',str(pathlib.Path(__file__)), 'mock', '--directory', str(target), '--certificates', str(certificates)],stdout=mocklog,stderr=subprocess.STDOUT)
        time.sleep(.3)
        env=os.environ.copy(); env['SSL_CERT_FILE']=str(certificates/'mock-ca.pem'); env['LITELLM_DATA_DIR']=str(target/'data')
        with (target/'gateway.log').open('w') as log:
            process=subprocess.Popen([a.binary,'--config',str(cfg)],env=env,stdout=log,stderr=subprocess.STDOUT)
            try:
                report=smoke(a.gateway,database=target/'smoke.sqlite'); (target/'results.json').write_text(json.dumps(report,indent=2)); print(json.dumps(report,indent=2))
            finally:
                process.terminate()
                try: process.wait(timeout=10)
                except subprocess.TimeoutExpired: process.kill(); process.wait()
                mock.terminate(); mock.wait(timeout=5); mocklog.close()
