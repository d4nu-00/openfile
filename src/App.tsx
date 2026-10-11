import {useState} from 'react';
import {backend,type WorkbenchApi} from './api/client';
export function App({api=backend}:{api?:WorkbenchApi}) {
  const [message,setMessage]=useState('Open a CBH database to begin.');
  const [busy,setBusy]=useState(false);
  async function openDatabase() {
    setBusy(true);
    let databaseId:string|undefined;
    try {
      const path=await api.chooseDatabase();if(typeof path!=='string')return;
      const db=await api.openDatabase(path,crypto.randomUUID());
      databaseId=db.databaseId;
      const page=await api.readPage(db.databaseId,'1',50,crypto.randomUUID());
      let selected='';
      const first=page.entries.find(e=>e.kind==='game');
      if(first){
        const loaded=await api.loadEntry(db.databaseId,first.ordinal,crypto.randomUUID());
        selected=loaded.game?` · Selected game: ${loaded.game.nodes.length-1} moves`:` · ${loaded.movesStatus}`;
        if(loaded.documentId)await api.closeDocument(loaded.documentId);
      }
      setMessage(`${db.entryCount} entries · ${page.entries.length} on this page${selected}`);
    } catch(error) {setMessage((error as {message?:string}).message??String(error));}
    finally {if(databaseId)await api.closeDatabase(databaseId).catch(()=>{});setBusy(false);}
  }
  return <main><h1>Chess Workbench</h1><button disabled={busy} onClick={openDatabase}>Open database</button><p role="status">{message}</p></main>;
}
