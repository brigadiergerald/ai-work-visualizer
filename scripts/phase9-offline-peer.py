"""Local deterministic app-server fixture. No networking, AI or external UI."""
import json,sys,pathlib,hashlib
root=pathlib.Path(sys.argv[1]);assert (root/'.ai-guild-qa').is_file()
thread='PHASE9_OFFLINE_THREAD';turns=[]
def send(v): print(json.dumps(v,ensure_ascii=False),flush=True)
def record(v):
    with (root/'offline-peer.jsonl').open('a',encoding='utf-8') as f:f.write(json.dumps(v,ensure_ascii=False)+'\n')
for line in sys.stdin:
    v=json.loads(line);method=v.get('method');mid=v.get('id');params=v.get('params',{})
    if (root/'offline-recovery-capture.json').is_file():
        capture=json.loads((root/'offline-recovery-capture.json').read_text(encoding='utf-8'))
        record({'classification':'OFFLINE_RECOVERY_FIXTURE','method':method,'inference':False})
        result={'thread/read':capture['thread'],'thread/turns/list':capture['turns'],'thread/items/list':capture['items'].get(params.get('turnId'))}.get(method)
        if result is None:send({'id':mid,'error':{'code':-32601,'message':'Recovery fixture forbids all mutating methods'}})
        else:send({'id':mid,'result':result})
        continue
    if method=='initialize':
        if (root/'offline-protocol-error').exists():send({'id':mid,'error':{'code':-32601,'message':'OFFLINE unsupported protocol'}})
        else:send({'id':mid,'result':{'userAgent':'OFFLINE_PHASE9'}})
    elif method=='account/read':send({'id':mid,'result':json.loads((root/'offline-account.json').read_text(encoding='utf-8')) if (root/'offline-account.json').exists() else {'account':{'type':'chatgpt'}}})
    elif method in ['thread/start','thread/resume']:send({'id':mid,'result':{'thread':{'id':thread}}})
    elif method=='thread/loaded/list':send({'id':mid,'result':{'data':[thread]}})
    elif method=='command/exec':
        record({'classification':'OFFLINE_CODE_TEST_FIXTURE','method':method,'params':params,'inference':False})
        assert params['sandboxPolicy']['type']=='readOnly'
        assert params['sandboxPolicy']['access']['readableRoots']==[params['cwd']]
        assert params['timeoutMs']==20000
        expected=json.loads((root/'offline-code-test.json').read_text(encoding='utf-8'))
        if 'error' in expected:send({'id':mid,'error':expected['error']})
        else:send({'id':mid,'result':expected})
    elif method=='turn/start':
        assert params['threadId']==thread
        index=len(turns);turn='PHASE9_OFFLINE_TURN_'+str(index+1);turns.append(turn)
        prompt='\n'.join(i.get('text','') for i in params['input'])
        three_person=(root/'offline-research.json').is_file()
        if three_person:
            assert index<3,'No additional turn is allowed in this fixture'
            raw=(root/['offline-research.json','offline-plan.json','offline-event.html'][index]).read_text(encoding='utf-8')
            record({'classification':'OFFLINE_THREE_PERSON_INPUT','event':'research-input-check','turnIndex':index,'researchReceived':index==0 or (root/'offline-research.json').read_text(encoding='utf-8') in prompt,'inference':False})
        else:raw=(root/('offline-plan.json' if index==0 else 'offline-event.html')).read_text(encoding='utf-8')
        record({'classification':'OFFLINE_FIXTURE','event':'turn/start','threadId':thread,'turnId':turn,'promptSha256':hashlib.sha256(prompt.encode()).hexdigest(),'fullPlanReceived':index==0 or (root/'offline-plan.json').read_text(encoding='utf-8') in prompt,'sandboxPolicy':params.get('sandboxPolicy'),'outputSchema':params.get('outputSchema'),'inference':False})
        send({'id':mid,'result':{'turn':{'id':turn,'status':'inProgress'}}})
        if index==0 and (root/'offline-question-request.json').is_file():
            question=json.loads((root/'offline-question-request.json').read_text(encoding='utf-8'))
            send({'id':8100,'method':'item/tool/requestUserInput','params':{'threadId':thread,'turnId':turn,'questions':question}})
            response=json.loads(sys.stdin.readline())
            assert response.get('id')==8100
            record({'classification':'OFFLINE_FIXTURE','event':'question-response','response':response,'inference':False})
        send({'method':'item/completed','params':{'threadId':thread,'turnId':turn,'item':{'type':'agentMessage','id':'OFFLINE_FINAL','text':raw,'phase':'final_answer'}}})
        send({'method':'turn/completed','params':{'threadId':thread,'turn':{'id':turn,'status':'completed'}}})
    elif mid is not None:send({'id':mid,'error':{'code':-32601,'message':'Unsupported OFFLINE fixture method'}})
