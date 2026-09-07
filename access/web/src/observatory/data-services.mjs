import {KnowledgeClient,RequestError} from './knowledge-client.mjs';
import {createToSQueryOperations} from '../query-operations';

// One page-owned connection for the scene and every reading/research tool.
// Request cancellation stays local to each consumer; responses and source
// identities are still checked by their existing contract adapters.
export function createObservatoryData({fetcher,timeoutMs=60000}={}){
  const client=new KnowledgeClient({fetcher,timeoutMs});
  const transport=new KnowledgeClient({fetcher,timeoutMs,base:''});
  const queries=createToSQueryOperations(async(url,options)=>{
    try{return await transport.request(url,options);}
    catch(error){
      if(error instanceof RequestError&&error.status===404)
        throw new RequestError(404,'Этот материал пока недоступен в выбранном способе просмотра.');
      throw error;
    }
  });
  return Object.freeze({client,queries});
}
