export function submissionBlocked({pending,busy,historical,responsePending,initial,approval,locked,composing}:{pending:boolean;busy:boolean;historical:boolean;responsePending:boolean;initial:boolean;approval:boolean;locked:boolean;composing:boolean}){
 return pending||busy||historical||responsePending||composing||(initial&&!approval&&locked);
}
