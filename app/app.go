package app

import (
	"context"
	"fmt"
	"net/http"

	"profile-views-counter/conf"

	"github.com/jackc/pgx/v4/pgxpool"
	"github.com/pkg/errors"
	"go.uber.org/zap"
)

type App struct {
	Config     *conf.Config
	Logger     *zap.Logger
	DbRegistry *DBRegistry
	HTTPServer *http.Server
}

type DBRegistry struct {
	Pool *pgxpool.Pool
}

func NewDBRegistry(db *pgxpool.Pool) *DBRegistry {
	return &DBRegistry{Pool: db}
}

func New(cfg *conf.Config, logger *zap.Logger) (*App, error) {
	logger.Info("connecting to database")
	dbPool, err := pgxpool.Connect(context.Background(), cfg.DatabaseURL)
	if err != nil {
		return nil, errors.Wrap(err, "failed to connect to postgres database")
	}

	app := &App{
		Config:     cfg,
		Logger:     logger,
		DbRegistry: NewDBRegistry(dbPool),
		HTTPServer: &http.Server{
			Addr: fmt.Sprintf(":%d", cfg.Port),
		},
	}

	return app, nil
}

func (a *App) Start() chan error {
	a.Logger.Info("starting server", zap.String("port", a.HTTPServer.Addr))
	rtn := make(chan error)
	go func() {
		rtn <- a.HTTPServer.ListenAndServe()
	}()
	return rtn
}

func (a *App) ShutDown() {
	a.Logger.Info("shutting down application")
	a.Logger.Info("closing database connection")
	a.DbRegistry.Pool.Close()
}
