package app

import (
	"profile-views-counter/app/routes/stats"

	"github.com/jackc/pgx/v4/pgxpool"
)

type DBRegistry struct {
	Pool *pgxpool.Pool
}

func NewDBRegistry(db *pgxpool.Pool) *DBRegistry {
	return &DBRegistry{Pool: db}
}

type ServiceRegistry struct {
	StatsService *stats.Service
}

func NewServiceRegistry(dbr *DBRegistry) *ServiceRegistry {
	return &ServiceRegistry{StatsService: stats.NewService(dbr.Pool)}
}
