-- Undead Horizon schema. Idempotent: safe to re-run.
-- Users themselves live in neon_auth."user" (Neon Managed Auth); user_id = that id.

create table if not exists profiles (
  user_id      text primary key,
  display_name text not null,
  coins        integer not null default 0 check (coins >= 0),
  owned        text[] not null default array['gun:pistol','perk:none','skin:default','hat:none','tracer:default'],
  equipped     jsonb not null default '{"gun":"pistol","perk":"none","skin":"default","hat":"none","tracer":"default"}',
  campaign     jsonb not null default '{"unlocked":0,"cleared":[]}',
  stats        jsonb not null default '{"runs":0,"kills":0,"bestWave":0}',
  imported     boolean not null default false,
  created_at   timestamptz not null default now(),
  updated_at   timestamptz not null default now()
);

-- One resumable save slot per player.
create table if not exists saves (
  user_id    text primary key references profiles(user_id) on delete cascade,
  data       text not null check (length(data) <= 4096),
  summary    jsonb not null,
  created_at timestamptz not null default now()
);

-- Every finished run (both modes), used for stats and the leaderboard.
create table if not exists runs (
  id         bigserial primary key,
  user_id    text not null references profiles(user_id) on delete cascade,
  name       text not null,
  mode       text not null check (mode in ('infinite','campaign')),
  chapter    integer not null default 0,
  diff       text not null check (diff in ('rookie','survivor','veteran','nightmare')),
  gun        text not null,
  wave       integer not null check (wave between 1 and 10000),
  level      integer not null check (level between 1 and 10000),
  kills      integer not null check (kills between 0 and 10000000),
  time       real not null check (time >= 0),
  coins      integer not null default 0,
  victory    boolean not null default false,
  abandoned  boolean not null default false,
  created_at timestamptz not null default now()
);

create index if not exists runs_leaderboard_idx
  on runs (wave desc, level desc, kills desc, time asc)
  where mode = 'infinite' and not abandoned;
create index if not exists runs_leaderboard_diff_idx
  on runs (diff, wave desc, level desc, kills desc, time asc)
  where mode = 'infinite' and not abandoned;
create index if not exists runs_user_idx on runs (user_id, created_at desc);
