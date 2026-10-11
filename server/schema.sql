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

-- ---------------------------------------------------------------- social / co-op
-- Short shareable code used to add friends (profiles.display_name is not unique).
alter table profiles add column if not exists friend_code text unique;
update profiles set friend_code = upper(substr(md5(user_id || random()::text), 1, 6)) where friend_code is null;

create table if not exists friends (
  user_id    text not null references profiles(user_id) on delete cascade,
  friend_id  text not null references profiles(user_id) on delete cascade,
  status     text not null check (status in ('pending','accepted')),
  created_at timestamptz not null default now(),
  primary key (user_id, friend_id)
);

-- Updated by the WebSocket heartbeat; "online" = seen in the last minute.
create table if not exists presence (
  user_id   text primary key references profiles(user_id) on delete cascade,
  last_seen timestamptz not null default now()
);

create table if not exists parties (
  id         bigserial primary key,
  host_id    text not null references profiles(user_id) on delete cascade,
  diff       text not null default 'survivor' check (diff in ('rookie','survivor','veteran','nightmare')),
  status     text not null default 'open' check (status in ('open','starting','in_game')),
  seed       bigint,
  created_at timestamptz not null default now()
);

create table if not exists party_members (
  party_id  bigint not null references parties(id) on delete cascade,
  user_id   text not null references profiles(user_id) on delete cascade,
  ready     boolean not null default false,
  joined_at timestamptz not null default now(),
  primary key (party_id, user_id)
);
create unique index if not exists party_members_one_party on party_members (user_id);

create table if not exists party_invites (
  party_id   bigint not null references parties(id) on delete cascade,
  from_id    text not null references profiles(user_id) on delete cascade,
  to_id      text not null references profiles(user_id) on delete cascade,
  created_at timestamptz not null default now(),
  primary key (party_id, to_id)
);

-- Cross-isolate fan-out: every WebSocket isolate polls this for its connected users.
create table if not exists events (
  id         bigserial primary key,
  user_id    text not null,
  payload    jsonb not null,
  created_at timestamptz not null default now()
);
create index if not exists events_user_idx on events (user_id, id);

alter table runs add column if not exists players integer not null default 1;
alter table party_members add column if not exists look jsonb not null default '{}';
alter table parties add column if not exists token text not null default substr(md5(random()::text), 1, 12);

-- Two save slots per player: one Infinite run and one Campaign run.
alter table saves add column if not exists slot text not null default 'infinite' check (slot in ('infinite','campaign'));
update saves set slot = coalesce(summary->>'mode', 'infinite') where slot is distinct from coalesce(summary->>'mode', 'infinite');
alter table saves drop constraint if exists saves_pkey;
alter table saves add constraint saves_pkey primary key (user_id, slot);

-- Chat (party text chat) + voice presence. Messages are not stored; only reports keep a copy.
alter table party_members add column if not exists voice boolean not null default false;
create table if not exists blocks (
  user_id    text not null references profiles(user_id) on delete cascade,
  blocked_id text not null references profiles(user_id) on delete cascade,
  created_at timestamptz not null default now(),
  primary key (user_id, blocked_id)
);
create table if not exists reports (
  id          bigserial primary key,
  reporter_id text not null references profiles(user_id) on delete cascade,
  reported_id text not null references profiles(user_id) on delete cascade,
  reason      text not null default '',
  message     text not null default '',
  created_at  timestamptz not null default now()
);

-- Client error reports (panic hook + window.onerror), for live debugging.
create table if not exists client_errors (
  id         bigserial primary key,
  user_id    text,
  kind       text not null default 'error',
  message    text not null default '',
  stack      text not null default '',
  url        text not null default '',
  ua         text not null default '',
  build      text not null default '',
  created_at timestamptz not null default now()
);
create index if not exists client_errors_time_idx on client_errors (created_at desc);
