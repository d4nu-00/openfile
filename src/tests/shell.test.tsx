import {render,screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {describe,it,expect,vi} from 'vitest';
import {App} from '../App';
import {backend} from '../api/client';
describe('desktop shell',()=>{
 it('loads a supported selected game through the command facade',async()=>{
  const api={...backend,chooseDatabase:vi.fn().mockResolvedValue('/synthetic/test.cbh'),openDatabase:vi.fn().mockResolvedValue({databaseId:'db-1',entryCount:'1'}),readPage:vi.fn().mockResolvedValue({entries:[{kind:'game',ordinal:'1'}]}),loadEntry:vi.fn().mockResolvedValue({documentId:'doc-1',game:{nodes:[{},{},{}]}}),closeDocument:vi.fn().mockResolvedValue(undefined),closeDatabase:vi.fn().mockResolvedValue(undefined)};
  render(<App api={api}/>);await userEvent.click(screen.getByRole('button',{name:'Open database'}));
  expect(await screen.findByText(/Selected game: 2 moves/)).toBeVisible();
  expect(api.closeDocument).toHaveBeenCalledWith('doc-1');
 });
 it('mounts and reaches injected database commands',async()=>{
  const api={...backend,chooseDatabase:vi.fn().mockResolvedValue('/synthetic/test.cbh'),openDatabase:vi.fn().mockResolvedValue({databaseId:'db-1',entryCount:'9007199254740993'}),readPage:vi.fn().mockResolvedValue({entries:[{}]}),closeDatabase:vi.fn().mockResolvedValue(undefined)};
  render(<App api={api}/>);await userEvent.click(screen.getByRole('button',{name:'Open database'}));
  expect(await screen.findByText('9007199254740993 entries · 1 on this page')).toBeVisible();
  expect(api.closeDatabase).toHaveBeenCalledWith('db-1');
 });
 it('displays a backend error',async()=>{
  const api={...backend,chooseDatabase:vi.fn().mockResolvedValue('/missing/test.cbh'),openDatabase:vi.fn().mockRejectedValue({code:'io',message:'Source unavailable'})};
  render(<App api={api}/>);await userEvent.click(screen.getByRole('button',{name:'Open database'}));
  expect(await screen.findByText('Source unavailable')).toBeVisible();
 });
});
