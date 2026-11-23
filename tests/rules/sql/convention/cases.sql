select 1 union select 2;
--       ^^^^^ union-distinct
select 1 union all select 2;
select 1 union distinct select 2;
select ifnull(a, 0), nvl(b, 0), coalesce(c, 0) from t;
--     ^^^^^^ prefer-coalesce
--                   ^^^ prefer-coalesce
select count(1), count(0), count(*), count(a) from t;
--     ^^^^^^^^ count-star
--               ^^^^^^^^ count-star
select a from t right join u on t.a = u.a;
--              ^^^^^ right-join
select a from t left join u on t.a = u.a;
select case when a then 1 else null end, case when a then 1 else 2 end from t;
--                        ^^^^^^^^^ else-null
select a b, c as d from t u join v as w on u.a = w.a;
--       ^ implicit-alias
--                        ^ implicit-alias
--                          ^^^^ inner-join
select a from t join u on t.a = u.a inner join v on v.a = t.a;
--              ^^^^ inner-join
select 1 ;
--      ^^ detached-semicolon
select 2
--      ^ detached-semicolon
;
