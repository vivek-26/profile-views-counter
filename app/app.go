package app

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httputil"
	"net/url"
	"time"

	"profile-views-counter/conf"

	"github.com/gorilla/mux"
	"github.com/jackc/pgx/v4/pgxpool"
	"github.com/pkg/errors"
	"go.uber.org/zap"
)

type App struct {
	Config                *conf.Config
	Logger                *zap.Logger
	DbRegistry            *DBRegistry
	ServiceRegistry       *ServiceRegistry
	HTTPServer            *http.Server
	ShieldsIOReverseProxy *httputil.ReverseProxy
}

func New(cfg *conf.Config, logger *zap.Logger) (*App, error) {
	logger.Info("connecting to database")
	dbPool, err := pgxpool.Connect(context.Background(), cfg.DatabaseURL)
	if err != nil {
		return nil, errors.Wrap(err, "failed to connect to postgres database")
	}

	badgeURL, err := url.Parse("https://img.shields.io/static/v1")
	if err != nil {
		return nil, errors.Wrap(err, "incorrect shields.io url")
	}

	badgeReverseProxy := httputil.NewSingleHostReverseProxy(badgeURL)
	badgeReverseProxy.Director = func(req *http.Request) {
		req.Header.Add("X-Forwarded-Host", req.Host)
		req.Header.Add("X-Origin-Host", badgeURL.Host)
		req.Header.Add("Cache-Control", "no-cache")
		req.URL.Scheme = badgeURL.Scheme
		req.URL.Host = badgeURL.Host
		req.Host = badgeURL.Host
		req.URL.Path = badgeURL.Path
	}
	badgeReverseProxy.Transport = &http.Transport{
		MaxIdleConnsPerHost: 20,
		MaxConnsPerHost:     20,
		IdleConnTimeout:     12 * time.Hour,
	}

	dbRegistry := NewDBRegistry(dbPool)
	serviceRegistry := NewServiceRegistry(dbRegistry, badgeReverseProxy)
	router := mux.NewRouter()
	RegisterRoutes(router, serviceRegistry)

	app := &App{
		Config:          cfg,
		Logger:          logger,
		DbRegistry:      dbRegistry,
		ServiceRegistry: serviceRegistry,
		HTTPServer: &http.Server{
			Addr:    fmt.Sprintf(":%d", cfg.Port),
			Handler: router,
		},
		ShieldsIOReverseProxy: badgeReverseProxy,
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

func RegisterRoutes(router *mux.Router, sr *ServiceRegistry) {
	router.HandleFunc("/stats/{service}/{user}/count.svg", sr.StatsService.Handler).
		Methods(http.MethodGet).Name("ProfileCountBadge")
}
