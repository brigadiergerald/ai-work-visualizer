import {useState} from 'react';
import {command,isNative} from './store';
import type {Snapshot,ProductSettings} from './model';

type Inspection={available:boolean;path?:string;version?:string;error?:string};
type Details={authType:string;planType?:string;models:{id:string;name?:string;isDefault?:boolean}[];limits?:unknown};
export function SetupPanel({snapshot,locked,onMode}:{snapshot:Snapshot;locked:boolean;onMode:(mode:'real'|'demo')=>void}){
 const [form,setForm]=useState<ProductSettings>(snapshot.settings);
 const [pending,setPending]=useState(false);const [notice,setNotice]=useState('');const [error,setError]=useState('');
 const [inspection,setInspection]=useState<Inspection|null>(null);const [details,setDetails]=useState<Details|null>(null);
 const change=(key:keyof ProductSettings,value:string)=>setForm(v=>({...v,[key]:value}));
 const perform=async(task:()=>Promise<void>)=>{if(pending)return;setPending(true);setNotice('');setError('');try{await task();}catch(e){setError(String(e));}finally{setPending(false);}};
 return <div className="product-setup">
  <p>自分のCodexを接続し、指示から成果物を作ります。設定の保存と接続テストではAI推論を開始しません。</p>
  <fieldset disabled={pending||locked||!isNative}>
   <legend>作業環境</legend>
   <label>Codex実行ファイル（空欄は自動検出）<input aria-label="Codex実行ファイル" value={form.codexPath} onChange={e=>change('codexPath',e.target.value)} placeholder="codex.exeの絶対パス" spellCheck={false}/></label>
   <button className="button small" onClick={()=>perform(async()=>{const result=await command<Inspection>('inspect_codex',{path:form.codexPath});setInspection(result);if(!result.available)throw new Error(result.error);setNotice('バージョンを確認しました。認証とプロトコルは接続テストで確認します。');})}>導入・バージョンを確認</button>
   {inspection?.available&&<p className="setup-result">{inspection.version}<br/><code>{inspection.path}</code></p>}
   <label>作業フォルダ（既存の絶対パス）<input aria-label="作業フォルダ" value={form.workRoot} onChange={e=>change('workRoot',e.target.value)} spellCheck={false}/></label>
   <label>成果物の納品先（既存の絶対パス）<input aria-label="成果物の納品先" value={form.outputRoot} onChange={e=>change('outputRoot',e.target.value)} spellCheck={false}/></label>
   <p>納品先の quests / Job ID に仕事ごとのファイルを保存します。入力元は変更しません。</p>
   <label>入力ファイル（任意・作業フォルダ内・1行1パス）<textarea aria-label="入力ファイル" rows={2} value={(form.inputPaths??[]).join('\n')} onChange={e=>setForm(v=>({...v,inputPaths:e.target.value.split('\n').filter(p=>p.trim())}))} spellCheck={false}/></label>
   <p>UTF-8の文書・JSON・CSV・ソースを12件まで、各2MBまで。開始時にコピーし、Codexへ全文を渡します。秘密情報を含むファイルは指定しないでください。</p>
   <label>使用モデル<select aria-label="使用モデル" value={form.model} onChange={e=>change('model',e.target.value)}><option value="">利用者のCodex設定に従う</option>{form.model&&!details?.models.some(m=>m.id===form.model)&&<option value={form.model}>{form.model}</option>}{details?.models.map(m=><option key={m.id} value={m.id}>{m.name||m.id}{m.isDefault?'（既定）':''}</option>)}</select></label>
   <p>モデル一覧は接続後に取得します。表示されたモデルの実利用権限は、推論なしでは確定できません。</p>
   <h3>実行権限と承認</h3>
   <p>Codexは読み取り専用・on-request承認で実行し、アプリが新しい成果物を保存します。公開・購入・元ファイル削除・依存ソフトの自動導入は実行しません。</p>
   <button className="button primary" onClick={()=>perform(async()=>{await command('save_setup',{settings:{...form,approvalPolicy:'on-request'}});setNotice('設定を保存しました。接続テストで認証を確認できます。');setDetails(null);})}>設定を保存</button>
   <button className="button" disabled={!snapshot.settings.setupComplete} onClick={()=>perform(async()=>{await command('connect_codex');const result=await command<Details>('connection_details');setDetails(result);setNotice('app-serverと認証を確認しました。新しい仕事は「クエスト開始」で実行します。');})}>接続テスト（推論なし）</button>
  </fieldset>
  {pending&&<p role="status">確認中です…</p>}{notice&&<p className="setup-result" role="status">{notice}</p>}{error&&<p className="error-inline" role="alert">{error}</p>}
  {snapshot.connection.error&&<p className="error-inline">{snapshot.connection.error}</p>}
  {details&&<div className="settings-section"><h3>正式に取得できた接続情報</h3><p>認証：{details.authType} · プラン：{details.planType||'取得できませんでした'}</p><details><summary>利用枠の応答</summary><pre>{details.limits?JSON.stringify(details.limits,null,2):'この環境では取得できませんでした'}</pre></details></div>}
  <div className="settings-section"><h3>実行モード</h3><div className="segmented"><button className={snapshot.mode==='real'?'selected':''} disabled={locked||pending} onClick={()=>onMode('real')}>Codex 実行</button><button className={snapshot.mode==='demo'?'selected':''} disabled={locked||pending} onClick={()=>onMode('demo')}>デモ</button></div><p>デモはAI推論を行わず、すべての工程と成果物にデモと表示します。</p></div>
  <div className="settings-section"><h3>この版でできる仕事</h3><p>文書・静的HTML・Markdown・JSON・CSV/TSV・ソースファイルの制作と、指定したテキストファイルの加工。複数Jobを保存し、1件ずつ実行します。</p><p>Python unittest・Node.js node:testは、計画に指定された場合だけCodexの読み取り専用sandboxで実行します。利用者側の対応runtimeが必要です。画像・動画生成、SNS投稿、外部操作、依存install、その他の実行方式は未対応です。完成品の意味・表示確認は別に行ってください。</p><h3>保存データ</h3><code>{snapshot.dataPath}</code><p>認証トークンはCodexが管理します。このアプリでは保存しません。別のCodexセッションは監視しません。</p></div>
  <p className="settings-note">開発candidateです。商用認証・販売条件は未解決で、販売可能との判定はしていません。</p>
 </div>;
}
