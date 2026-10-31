"""Explicit OFFLINE stdio peer. No network, model, account or inference access.
Request/response shapes follow the installed app-server generated types.
Never records answer values, including disposable secret answers.
"""
import json, pathlib, sys, threading, time, uuid
ROOT=pathlib.Path(sys.argv[1]).resolve()
assert (ROOT/'.ai-guild-qa').is_file()
lock=threading.RLock()
pending={}; turn=None; thread='mock-thread-'+str(uuid.uuid4()); seq=0; loaded_only=False
journal=ROOT/'mock-peer.jsonl'
def record(**v):
    with journal.open('a',encoding='utf-8') as f:
        f.write(json.dumps({'at':time.time(),'transport':'OFFLINE_MOCK','inference':False,**v},ensure_ascii=False)+'\n')
def send(v):
    with lock: print(json.dumps(v,ensure_ascii=False),flush=True)
def event(method,params): send({'method':method,'params':params})
def complete(text='MOCK_OFFLINE_RESULT'):
    global turn
    if turn is None:return
    event('item/completed',{'threadId':thread,'turnId':turn,'item':{'type':'agentMessage','id':'final','text':text,'phase':'final_answer'}})
    event('turn/completed',{'threadId':thread,'turn':{'id':turn,'status':'completed','error':None}})
    record(event='formal_completed',threadId=thread,turnId=turn)
    turn=None
def request(kind,case):
    rid='mock-request-'+str(uuid.uuid4()); item='mock-item-'+str(uuid.uuid4())
    params={'threadId':thread,'turnId':turn,'itemId':item}
    if kind=='questions':
        method='item/tool/requestUserInput'
        params.update(questions=[{'id':'format','header':'形式','question':'MOCK：出力形式は？','isSecret':False,'isOther':True,'options':[{'label':'Markdown','description':'独立した形式の回答'}]},{'id':'audience','header':'読者','question':'MOCK：対象読者は？','isSecret':False,'isOther':True,'options':None},{'id':'private','header':'ダミー','question':'MOCK：使い捨て秘密入力','isSecret':True,'isOther':True,'options':None}],isBlocking=True,autoResolutionMs=None)
    elif kind=='command':
        method='item/commandExecution/requestApproval'
        params.update(kind='command',startedAtMs=int(time.time()*1000),environmentId=None,command='MOCK ONLY: create '+case+'.txt',cwd=str(ROOT),reason='Offline mock approval '+case,commandActions=[])
    else:
        method='item/fileChange/requestApproval'
        params.update(startedAtMs=int(time.time()*1000),reason='Offline mock file approval '+case)
        event('item/started',{'threadId':thread,'turnId':turn,'item':{'type':'fileChange','id':item,'changes':[{'path':str(ROOT/(case+'.txt')),'kind':{'type':'add'},'diff':'+MOCK_APPROVED_FILE\n'}],'status':'inProgress'}})
    pending[rid]={'kind':kind,'case':case,'params':params,'responses':0}
    send({'id':rid,'method':method,'params':params})
    record(event='request',requestId=rid,kind=kind,case=case,threadId=thread,turnId=turn,questionIds=['format','audience','private'] if kind=='questions' else [])
    return rid
def expire(rid):
    time.sleep(.7)
    with lock:
        if rid in pending:
            pending.pop(rid)
            event('serverRequest/resolved',{'threadId':thread,'requestId':rid})
            record(event='expired',requestId=rid)
            complete('MOCK_EXPIRED_REQUEST')
def respond(v):
    global turn
    rid=v['id']; p=pending.get(rid)
    if p is None:
        record(event='unexpected_or_duplicate_response',requestId=rid)
        return
    p['responses']+=1
    result=v.get('result',{})
    record(event='response',requestId=rid,kind=p['kind'],case=p['case'],responseCount=p['responses'],decision=result.get('decision'),questionIds=sorted(result.get('answers',{})),threadId=thread,turnId=turn)
    if p['kind']=='questions':
        a=result.get('answers',{})
        ok=set(a)=={'format','audience','private'} and a['format']['answers']==['Markdown'] and a['audience']['answers']==['QA audience'] and bool(a['private']['answers'][0])
        record(event='question_mapping_check',ok=ok,secretReceived=bool(a.get('private',{}).get('answers')))
        text='MOCK echo for persistence test: '+a.get('private',{}).get('answers',[''])[0]
    else:
        accepted=result.get('decision')=='accept'
        if accepted: (ROOT/(p['case']+'.txt')).write_text('MOCK_APPROVED_FILE\n',encoding='utf-8')
        record(event='operation_outcome',case=p['case'],executed=accepted,targetExists=(ROOT/(p['case']+'.txt')).exists())
        text='MOCK_APPROVAL_RESULT'
    pending.pop(rid,None)
    # Deliberately leave time for a duplicate UI/Rust call to race.
    def finish():
        time.sleep(.25)
        with lock:
            event('serverRequest/resolved',{'threadId':thread,'requestId':rid})
            if not pending:complete(text)
    threading.Thread(target=finish,daemon=True).start()
for line in sys.stdin:
    try:
        v=json.loads(line)
        with lock:
            method=v.get('method'); mid=v.get('id')
            if not method:
                respond(v); continue
            if method=='initialize':send({'id':mid,'result':{'userAgent':'OFFLINE_MOCK'}})
            elif method=='initialized':pass
            elif method=='account/read':send({'id':mid,'result':{'account':{'type':'chatgpt'},'requiresOpenaiAuth':True}})
            elif method=='thread/start':
                thread='mock-thread-'+str(uuid.uuid4())
                send({'id':mid,'result':{'thread':{'id':thread}}})
            elif method=='thread/resume':
                if loaded_only:send({'id':mid,'error':{'code':-32600,'message':'no rollout found for loaded ephemeral thread'}})
                else:
                    thread=v['params']['threadId']
                    send({'id':mid,'result':{'thread':{'id':thread}}})
                record(event='disk_resume_attempt')
            elif method=='thread/loaded/list':
                send({'id':mid,'result':{'data':[thread],'nextCursor':None}})
                record(event='loaded_list')
            elif method=='turn/start':
                seq+=1; turn='mock-turn-'+str(seq)
                prompt=' '.join(i.get('text','') for i in v['params'].get('input',[]))
                if 'QA_CASE=loaded-only' in prompt:loaded_only=True
                send({'id':mid,'result':{'turn':{'id':turn,'status':'inProgress','items':[],'error':None}}})
                record(event='mock_turn_start',threadId=thread,turnId=turn)
                if 'QA_CASE=two-requests' in prompt:
                    request('command','first');request('command','second')
                elif 'QA_CASE=stale' in prompt: threading.Thread(target=expire,args=(request('command','expired'),),daemon=True).start()
                elif 'QA_CASE=questions' in prompt:request('questions','questions')
                elif 'QA_CASE=command' in prompt:request('command','command')
                elif 'QA_CASE=file' in prompt:request('file','file')
                elif 'QA_CASE=wrong-turn' in prompt:
                    event('item/agentMessage/delta',{'threadId':thread,'turnId':'different-turn','itemId':'wrong','delta':'WRONG_TURN_CONTENT'})
                    event('item/agentMessage/delta',{'threadId':thread,'turnId':turn,'itemId':'valid','delta':'MOCK_CURRENT_TURN'})
                    event('turn/completed',{'threadId':thread,'turn':{'id':turn,'status':'completed','error':None}});turn=None
                elif 'QA_CASE=wait' in prompt:request('command','wait')
                else:complete()
            elif method=='turn/interrupt':
                for rid in list(pending):event('serverRequest/resolved',{'threadId':thread,'requestId':rid})
                pending.clear(); send({'id':mid,'result':{}})
                event('turn/completed',{'threadId':thread,'turn':{'id':turn,'status':'interrupted','error':None}})
                record(event='formal_interrupted',threadId=thread,turnId=turn);turn=None
            elif method=='turn/steer':send({'id':mid,'result':{'turnId':turn}})
            else:send({'id':mid,'error':{'code':-32601,'message':'Offline mock method unavailable'}})
    except Exception as e:
        record(event='peer_error',errorType=type(e).__name__)
