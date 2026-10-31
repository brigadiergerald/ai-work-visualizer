import {t as tr} from '../../i18n';
import {Play} from 'lucide-react';
import type {PurposeDefinition,SocialTextInput} from '../../core/purposes';
import {socialInputIssueCode,utf8Bytes} from '../../core/purposes';
import {CompletionSelector} from '../IntegrationsPanel';
import type {CompletionPlanInput,IntegrationProfile} from '../../core/integrations';
export interface SocialTextFormProps{
 value:SocialTextInput;onChange:(v:SocialTextInput)=>void;definition:PurposeDefinition|undefined;
 completionPlan:CompletionPlanInput;onPlan:(p:CompletionPlanInput)=>void;profiles:IntegrationProfile[];
 busy:boolean;locked:boolean;canStart:boolean;startBlockedReason:string;onStart:()=>void;
}
export default function SocialTextForm({value,onChange,definition,completionPlan,onPlan,profiles,busy,locked,canStart,startBlockedReason,onStart}:SocialTextFormProps){
 const issueCode=socialInputIssueCode(value),issue=issueCode?tr(issueCode):'';
 return <form className="social-text-form" onSubmit={e=>{e.preventDefault();if(canStart&&!issue)onStart();}}>
  <p className="muted">{tr("social.001")}</p>
  <label htmlFor="social-theme">{tr("social.002")}<textarea id="social-theme" value={value.theme} onChange={e=>onChange({...value,theme:e.target.value})} placeholder={tr("social.009")} rows={4} required disabled={busy} aria-describedby="social-input-help"/></label>
  <label htmlFor="social-context">{tr("social.003")}<textarea id="social-context" value={value.context??''} onChange={e=>onChange({...value,context:e.target.value})} placeholder={tr("social.010")} rows={3} disabled={busy}/></label>

  <label htmlFor="content-language">{tr("social.004")}<select id="content-language" value={value.contentLanguage} onChange={e=>onChange({...value,contentLanguage:e.target.value as SocialTextInput['contentLanguage']})} disabled={busy}><option value="ja">{tr("dashboard.074")}</option><option value="en">{tr("social.005")}</option></select></label>

  <details><summary>{tr("social.006")}</summary><p id="social-input-help" className="hint">{tr("social.007")}{utf8Bytes(value.theme).toLocaleString()}{tr("social.014")}{utf8Bytes(value.context??'').toLocaleString()}{tr("social.015")}</p><p className="hint">{tr("social.008")}</p><CompletionSelector value={completionPlan} onChange={onPlan} profiles={profiles} disabled={busy||locked}/></details>
  <button className="primary purpose-start" type="submit" disabled={!canStart||!!issue||definition?.availability!=='READY'}><Play size={20}/>{busy?tr("social.011"):tr("social.012")}</button>
  <p className="hint" role="status">{issue||startBlockedReason||tr("social.013")}</p>
 </form>;
}
