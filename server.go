package main

import (
	"go.uber.org/zap"
	"profile-views-counter/conf"
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

	logger.Info("config loaded", zap.Any("config values", config))
}
