package main

import (
	"os"
	"os/signal"
	"syscall"

	"profile-views-counter/app"
	"profile-views-counter/conf"

	"go.uber.org/zap"
)

func main() {
	logger, err := zap.NewProduction()
	if err != nil {
		panic(err)
	}

	config, err := conf.Load()
	if err != nil {
		logger.Panic(err.Error())
	}

	appInst, err := app.New(config, logger)
	if err != nil {
		logger.Panic(err.Error())
		return
	}

	// start the server
	appErr := appInst.Start()

	// handle graceful shutdown
	killSigs := make(chan os.Signal, 1)
	signal.Notify(killSigs, os.Interrupt, syscall.SIGINT, syscall.SIGTERM, syscall.SIGABRT, syscall.SIGHUP)

	select {
	case killSig := <-killSigs:
		logger.Info("received kill signal", zap.String("signal", killSig.String()))
	case serveErr := <-appErr:
		logger.Error("server crashed", zap.String("error", serveErr.Error()))
	}

	appInst.ShutDown()
	logger.Info("application shutdown complete")
}
