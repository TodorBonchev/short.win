import {strict as assert} from 'node:assert';
import {test} from 'node:test';
import {rawAmount,formatAmount,claimable,depositShares,redemption,U64_MAX} from '../src/amounts.ts';
test('amount entry is exact beyond Number precision and refuses truncation',()=>{
 assert.equal(rawAmount('18446744073.709551615',9),U64_MAX);assert.equal(formatAmount(U64_MAX,9),'18446744073.709551615');
 assert.equal(rawAmount('0.000000001',9),1n);assert.equal(rawAmount('9007199254740993',0),9007199254740993n);
 for(const value of ['0.0000000001','1e3','-1','+1','NaN','1.','01','18446744073.709551616'])assert.throws(()=>rawAmount(value,9));
});
test('preview follows floor principal and actually paid income without time accrual',()=>{
 const pool={cash:4n,principalOut:6n,shares:6n,solReserve:5n,solIndex:[1_666_666_666_666_666_666n,0n,0n,0n]};
 assert.equal(depositShares(2n,pool),1n);assert.equal(redemption(3n,pool),5n);
 assert.equal(claimable(pool,{shares:3n,checkpoint:[0n,0n,0n,0n],fraction:0n,pendingSol:0n}),4n);
 assert.throws(()=>depositShares(1n,{cash:0n,principalOut:0n,shares:2n}));
 assert.throws(()=>claimable(pool,{shares:3n,checkpoint:[2_000_000_000_000_000_000n,0n,0n,0n],fraction:0n,pendingSol:0n}));
});
