import {coreLabel,formatDate,t as tr,message,uiMessage,type UiMessage} from '../i18n';
import {useEffect,useRef,useState} from 'react';
import type {CoreClient} from '../core/contract';
import {type Connector,type ConnectorAction,type ConnectorHub,type Transport,canCheck,canSendMock} from '../core/connectors';
import ProviderPanel from './ProviderPanel';
export default function ConnectorPanel({client}:{client:CoreClient}) {
 const [hub,setHub]=useState<ConnectorHub|null>(null),[selected,setSelected]=useState(''),[adding,setAdding]=useState(false);
 const [transport,setTransport]=useState<Transport>('api'),[preset,setPreset]=useState('buffer'),[name,setName]=useState(''),[url,setUrl]=useState('');
 const [auth,setAuth]=useState('oauth'),[scenario,setScenario]=useState('accepted'),[busy,setBusy]=useState(false),[error,setError]=useState(''),[notice,setNotice]=useState<UiMessage>('');
 const inFlight=useRef(false);const c=hub?.connectors.find(v=>v.id===selected);
 const refresh=async()=>{const next=await client.execute<ConnectorHub>({action:'connectors',request:{operation:'get'}});setHub(next);setSelected(prev=>next.connectors.some(c=>c.id===prev)?prev:next.connectors[0]?.id??'');};
 useEffect(()=>{if(client.environment==='desktop')void refresh().catch(e=>setError(String(e)));},[client]);
 async function run(request:ConnectorAction) {
  if(inFlight.current)return;inFlight.current=true;setBusy(true);setError('');setNotice('');
  try {const result=await client.execute<Connector>({action:'connectors',request});await refresh();if(result.id)setSelected(result.id);setNotice(message("connectorPanel.052"));if(request.operation==='register')setAdding(false);}
  catch(e){setError(String(e));}finally{inFlight.current=false;setBusy(false);}
 }
 function chooseTransport(t:Transport){setTransport(t);setPreset(t==='webhook'?'mock-webhook':'buffer');setAuth(t==='webhook'?'none':'oauth');}
 function register(){
  const mock=preset==='mock-webhook',mcpMock=preset==='mock-mcp';
  void run({operation:'register',displayName:name.trim()||(mock?'ローカルWebhook':mcpMock?'Mock MCP':'Buffer MCP'),transport,providerId:preset,
   endpointId:transport==='mcp'?(mcpMock?'offline:mock-mcp':'https://mcp.buffer.com/mcp'):(mock?'http://127.0.0.1:18791/connector-hub/mock':url.trim()),
   authMethod:mcpMock?'simulation':auth,settings:transport==='webhook'?{method:'POST',schema:'core-handoff-v1',timeoutMs:1000,scenario}:{}});
 }
 return <section className="panel integrations-panel connector-hub" aria-label="Connector Hub">
  <div className="section-heading"><span className="eyebrow">CONNECTOR HUB</span><h2>{tr("connectorPanel.001")}</h2><span className="muted">{tr("connectorPanel.002")}</span></div>
  <p>{tr("connectorPanel.003")}</p>
  {error&&<p className="error" role="alert">{error}</p>}{notice&&<p role="status">{uiMessage(notice)}</p>}
  {client.environment!=='desktop'?<p>{tr("connectorPanel.004")}</p>:<>
  <div className="action-row"><button className="primary" disabled={busy} aria-expanded={adding} onClick={()=>setAdding(v=>!v)}>{tr("connectorPanel.005")}</button><button disabled={busy} onClick={()=>void refresh().catch(e=>setError(String(e)))}>{tr("connectorPanel.006")}</button><span role="status">{busy?tr("connectorPanel.054"):tr("connectorPanel.055")}</span></div>
  {adding&&<div className="connector-add"><fieldset disabled={busy}><legend>{tr("connectorPanel.007")}</legend>
   <label>{tr("connectorPanel.008")}<select aria-label={tr("connectorPanel.008")} value={transport} onChange={e=>chooseTransport(e.target.value as Transport)}><option value="api">API</option><option value="mcp">MCP</option><option value="webhook">Webhook</option></select></label>
   {transport==='api'?<p>{tr("connectorPanel.009")}</p>:<>
   <label>{tr("connectorPanel.010")}<input aria-label={tr("connectorPanel.050")} maxLength={120} value={name} onChange={e=>setName(e.target.value)}/></label>
   {transport==='mcp'?<>
    <label>{tr("connectorPanel.011")}<select aria-label={tr("connectorPanel.011")} value={preset} onChange={e=>{setPreset(e.target.value);setAuth(e.target.value==='mock-mcp'?'simulation':'oauth');}}><option value="buffer">{tr("connectorPanel.012")}</option><option value="mock-mcp">{tr("connectorPanel.013")}</option></select></label>
    <p>{tr("connectorPanel.014")}{preset==='buffer'?'https://mcp.buffer.com/mcp':tr("connectorPanel.056")}</p>
    {preset==='buffer'&&<><label>{tr("connectorPanel.015")}<select aria-label={tr("connectorPanel.015")} value={auth} onChange={e=>setAuth(e.target.value)}><option value="oauth">{tr("connectorPanel.016")}</option><option value="apiKey">{tr("connectorPanel.017")}</option></select></label><p className="hint">{tr("connectorPanel.018")}</p></>}
   </>:<>
    <label>{tr("connectorPanel.019")}<select aria-label={tr("connectorPanel.019")} value={preset} onChange={e=>{setPreset(e.target.value);setAuth('none');}}><option value="mock-webhook">{tr("connectorPanel.020")}</option><option value="make">{tr("connectorPanel.021")}</option></select></label>
    {preset==='make'?<label>Webhook URL<input aria-label="Webhook URL" value={url} onChange={e=>setUrl(e.target.value)} placeholder="https://hook.eu1.make.com/…"/><small>{tr("connectorPanel.022")}</small></label>:<p>{tr("connectorPanel.023")}</p>}
    <label>{tr("connectorPanel.024")}<select aria-label={tr("connectorPanel.024")} value={auth} onChange={e=>setAuth(e.target.value)}><option value="none">{tr("connectorPanel.025")}</option>{preset==='mock-webhook'?<option value="syntheticBearer">{tr("connectorPanel.026")}</option>:<option value="bearer">{tr("connectorPanel.027")}</option>}</select></label>
    {preset==='mock-webhook'&&<label>{tr("connectorPanel.028")}<select aria-label={tr("connectorPanel.028")} value={scenario} onChange={e=>setScenario(e.target.value)}><option value="accepted">{tr("connectorPanel.029")}</option><option value="httpError">{tr("connectorPanel.030")}</option><option value="timeout">{tr("connectorPanel.031")}</option><option value="unknown">{tr("connectorPanel.032")}</option></select></label>}
    <p>{tr("connectorPanel.033")}</p>
   </>}
   <button className="primary" onClick={register}>{tr("connectorPanel.034")}</button></>}
  </fieldset>{transport==='api'&&<ProviderPanel client={client}/>}</div>}
  <div className="connector-layout"><nav aria-label={tr("connectorPanel.051")} className="connector-list">{hub?.connectors.length?hub.connectors.map(item=><button key={item.id} disabled={busy} aria-current={selected===item.id?'true':undefined} onClick={()=>setSelected(item.id)}><strong>{item.displayName}</strong><small>{item.transport.toUpperCase()} · {coreLabel('connector',item.state)}</small><small>{item.scope==='OFFLINE_SIMULATION'?tr("connectorPanel.057"):item.scope==='EXISTING_API_EVIDENCE'?tr("connectorPanel.058"):tr("connectorPanel.059")}</small></button>):<p>{tr("connectorPanel.035")}</p>}</nav>
   <div>{c&&<>
    <h3>{c.displayName}</h3><p role="status"><strong>{coreLabel('connector',c.state)}</strong> · {c.transport.toUpperCase()}</p><p>{c.reason}</p>
    <dl className="connector-facts"><dt>{tr("connectorPanel.036")}</dt><dd>{c.endpointId}</dd><dt>{tr("connectorPanel.037")}</dt><dd>{c.authMethod} · {c.credentialRef?tr("connectorPanel.060"):tr("connectorPanel.061")}</dd><dt>{tr("connectorPanel.038")}</dt><dd>{c.lastCheckedAt?formatDate(c.lastCheckedAt):tr("connectorPanel.062")}</dd><dt>{tr("connectorPanel.039")}</dt><dd>{c.scope}</dd></dl>
    <div className="action-row">{c.transport!=='webhook'&&<button disabled={busy||!canCheck(c)} onClick={()=>void run({operation:'check',connectorId:c.id,revision:c.revision})}>{c.transport==='mcp'?tr("connectorPanel.063"):tr("connectorPanel.064")}</button>}
    {c.providerId==='mock-webhook'&&<button disabled={busy||!canSendMock(c)} onClick={()=>void run({operation:'sendMock',connectorId:c.id,revision:c.revision})}>{tr("connectorPanel.040")}</button>}
    <button disabled={busy||c.state==='DISCONNECTED'} onClick={()=>void run({operation:'disconnect',connectorId:c.id,revision:c.revision})}>{tr("connectorPanel.041")}</button></div>
    <h3>{tr("connectorPanel.042")}</h3><ul>{Object.entries(c.capabilities).map(([k,on])=><li key={k}>{coreLabel('capability',k)}：{on?tr("connectorPanel.065"):tr("connectorPanel.066")}</li>)}</ul>
    {c.transport==='mcp'&&<><h3>{tr("connectorPanel.043")}{c.discovery.tools.length}{tr("connectorPanel.069")}</h3><p>{tr("connectorPanel.044")}</p><p>{tr("connectorPanel.045")}{c.discovery.protocolVersion??tr("connectorPanel.062")}</p>{c.discovery.tools.map(t=><details key={t.name}><summary>{t.name} · {t.category}{tr("connectorPanel.070")}</summary><p>{t.description}</p><p>{tr("connectorPanel.046")}{t.mappedCapability??tr("connectorPanel.067")}。{t.reason}</p><pre>{JSON.stringify(t.inputSchema,null,2)}</pre></details>)}</>}
    <details><summary>{tr("connectorPanel.047")}{c.attempts.length}{tr("connectorPanel.069")}</summary><ul>{c.attempts.map(a=><li key={a.id}>{a.status} · {a.startedAt}{a.receipt&&tr("connectorPanel.071", {p0: a.receipt.receipt, p1: a.receipt.httpStatus??'不明'})}</li>)}</ul></details>
    <small>{tr("connectorPanel.048")}{c.revision}{tr("connectorPanel.072")}{c.version}{tr("connectorPanel.073")}</small>
   </>}</div></div>
  <details><summary>{tr("connectorPanel.049")}</summary><ProviderPanel client={client}/></details>
  </>}
 </section>;
}
