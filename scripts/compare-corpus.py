#!/usr/bin/env python3
"""Private optional validation; requires python-chess and an explicit MIT reference.
Never bundles source games. The JSON report contains counts, timing and failures.
"""
import argparse
import hashlib
import json
import resource
import subprocess
import sys
from pathlib import Path
import chess
import chess.pgn

def canonical_reference(game):
    # Iterative traversal keeps deep mainlines off Python's call stack.
    rows = []
    stack = [(game, 0)]
    while stack:
        node, parent = stack.pop()
        if node is game:
            index = 0
        else:
            index = len(rows) + 1
            rows.append([parent, node.move.uci()])
        stack.extend((child, index) for child in reversed(node.variations))
    return rows

def canonical_native(game):
    rows = []
    stack = [(0, 0)]
    while stack:
        node_id, parent = stack.pop()
        node = game['nodes'][node_id]
        if node_id == 0:
            index = 0
        else:
            index = len(rows) + 1
            rows.append([parent, node['uci']])
        stack.extend((child, index) for child in reversed(node['children']))
    return rows

def initial(codec):
    board = [[(0, None) for _ in range(8)] for _ in range(8)]
    pieces = [None] + [[None] * 8 for _ in range(12)]
    for rank, back, pawns in [(0,[4,2,3,1,9,3,2,4],11),(7,[8,6,7,5,10,7,6,8],12)]:
        counts = {}
        for file, kind in enumerate(back):
            number = counts.get(kind, 0); counts[kind] = number + 1
            board[file][rank] = (kind, None if kind in (9,10) else number)
            pieces[kind][number] = (file,rank)
        for file in range(8):
            r = 1 if rank == 0 else 6
            board[file][r] = (pawns,file); pieces[pawns][file] = (file,r)
    return board, pieces

def main():
    p = argparse.ArgumentParser()
    p.add_argument('--path',type=Path,required=True)
    p.add_argument('--reference',type=Path,required=True)
    p.add_argument('--inspect',type=Path,required=True)
    p.add_argument('--observations',type=Path,required=True)
    p.add_argument('--report',type=Path,required=True)
    args = p.parse_args()
    product = Path(__file__).resolve().parents[1]
    if args.report.resolve().is_relative_to(product):
        p.error('Private reports must be outside the product repository')
    revision = subprocess.check_output(['git','-C',str(args.reference),'rev-parse','HEAD'],text=True).strip()
    if revision != '42b3592738062db1f768239e85df1b98cb1cead9':
        p.error('Reference must be the audited pinned revision')
    sys.path.insert(0,str(args.reference))
    import game as codec
    import header
    observations = json.loads(args.observations.read_text())
    researched = sorted({r['record_number'] for r in observations['new_header_records']})
    first = json.loads(subprocess.check_output([str(args.inspect),'--path',str(args.path),'--page-limit','50','--entry','14']))
    count = first['info']['entryCount']
    spread = [1 + i * (count-1)//199 for i in range(200)]
    report = {'referenceRevision':revision,'referenceSha256':hashlib.sha256((args.reference/'game.py').read_bytes()).hexdigest(),
              'researchedCount':len(researched),'spreadCount':len(set(spread)), 'supported':0,'unsupported':[], 'failures':[],
              'maxPageMs':first['metrics']['pageMs'],'maxLoadMs':0,'bytesRead':0}
    with args.path.open('rb') as cbh, args.path.with_suffix('.cbg').open('rb') as cbg:
        for ordinal in sorted(set(researched+spread)):
            output=json.loads(subprocess.check_output([str(args.inspect),'--path',str(args.path),'--page-limit','1','--page-start',str(ordinal),'--entry',str(ordinal)]))
            metrics=output['metrics']; report['bytesRead']+=metrics['bytesRead']
            report['maxPageMs']=max(report['maxPageMs'],metrics['pageMs']); report['maxLoadMs']=max(report['maxLoadMs'],metrics['loadMs'])
            loaded=output['loaded']; cbh.seek(ordinal*46); raw=cbh.read(46)
            if loaded['movesStatus']!='available':
                report['unsupported'].append({'ordinal':ordinal,'status':loaded['movesStatus'],'diagnostics':loaded['diagnostics']})
                if loaded['movesStatus']=='corrupt': report['failures'].append({'ordinal':ordinal,'reason':'native corrupt'})
                continue
            offset=int.from_bytes(raw[1:5],'big'); cbg.seek(offset); head=cbg.read(4)
            length=int.from_bytes(head[1:],'big')
            if length>4*1024*1024: raise ValueError('Reference record exceeds bound')
            record=head+cbg.read(length-4)
            try:
                flags=codec.get_info_gamelen(record,0)
                if flags[1] or flags[2] or flags[3]: raise ValueError('Native accepted unsupported reference flags')
                if flags[0]:
                    fen,board,pieces=codec.decode_start_position(record,0)
                    ref,error=codec.decode(record[32:],board,pieces,fen=fen)
                else:
                    board,pieces=initial(codec);ref,error=codec.decode(record[4:],board,pieces)
                if error: raise ValueError(error)
                native=loaded['game']
                if canonical_reference(ref)!=canonical_native(native): raise ValueError('ordered UCI tree differs')
                native_fen=chess.Board(native['start_fen']).fen(en_passant='fen')
                if ref.board().fen(en_passant='fen')!=native_fen: raise ValueError('initial FEN differs')
                year,month,day=header.get_yymmdd(raw)
                date='.'.join(f'{value:0{width}d}' if value else '?'*width for value,width in [(year,4),(month,2),(day,2)])
                result=header.get_result(raw)
                if native['tags']['Date']!=date: raise ValueError(f'date differs: {date}')
                if native['tags']['Result']!=result: raise ValueError(f'result differs: {result}')
                report['supported']+=1
            except Exception as error:
                report['failures'].append({'ordinal':ordinal,'reason':str(error)})
    report['peakChildRss']=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    args.report.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items() if k not in ['unsupported','failures']},indent=2))
    print(f"unsupported={len(report['unsupported'])}, failures={len(report['failures'])}")
    return bool(report['failures'])

if __name__ == '__main__': sys.exit(main())
