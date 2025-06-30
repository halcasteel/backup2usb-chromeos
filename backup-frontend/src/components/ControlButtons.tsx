import { memo } from 'react';
import {
  Box,
  Button,
  HStack,
  useToast,
} from '@chakra-ui/react';
import { FaPlay, FaPause, FaStop } from 'react-icons/fa';
import { BackupStatus } from '../types/backup';
import { startBackup, pauseBackup, stopBackup } from '../services/api';

interface ControlButtonsProps {
  status: BackupStatus | null;
}

export function ControlButtons({ status }: ControlButtonsProps) {
  const toast = useToast();

  const handleStart = async () => {
    try {
      await startBackup();
      toast({
        title: 'Backup started',
        status: 'success',
        duration: 3000,
        isClosable: true,
      });
    } catch (error) {
      toast({
        title: 'Failed to start backup',
        description: error instanceof Error ? error.message : 'Unknown error',
        status: 'error',
        duration: 5000,
        isClosable: true,
      });
    }
  };

  const handlePause = async () => {
    try {
      await pauseBackup();
      toast({
        title: 'Backup paused',
        status: 'info',
        duration: 3000,
        isClosable: true,
      });
    } catch (error) {
      toast({
        title: 'Failed to pause backup',
        description: error instanceof Error ? error.message : 'Unknown error',
        status: 'error',
        duration: 5000,
        isClosable: true,
      });
    }
  };

  const handleStop = async () => {
    try {
      await stopBackup();
      toast({
        title: 'Backup stopped',
        status: 'warning',
        duration: 3000,
        isClosable: true,
      });
    } catch (error) {
      toast({
        title: 'Failed to stop backup',
        description: error instanceof Error ? error.message : 'Unknown error',
        status: 'error',
        duration: 5000,
        isClosable: true,
      });
    }
  };

  const isRunning = status?.state === 'running';
  const isPaused = status?.state === 'paused';
  const isIdle = status?.state === 'stopped';

  return (
    <Box 
      position="fixed" 
      bottom={6} 
      right={6}
      zIndex={10}
    >
      <HStack spacing={3}>
        <Button
          size="lg"
          colorScheme="green"
          leftIcon={<FaPlay />}
          onClick={handleStart}
          isDisabled={isRunning}
          minW="120px"
        >
          START
        </Button>
        
        <Button
          size="lg"
          colorScheme="yellow"
          leftIcon={<FaPause />}
          onClick={handlePause}
          isDisabled={!isRunning}
          minW="120px"
        >
          PAUSE
        </Button>
        
        <Button
          size="lg"
          colorScheme="red"
          leftIcon={<FaStop />}
          onClick={handleStop}
          isDisabled={isIdle}
          minW="120px"
        >
          STOP
        </Button>
      </HStack>
    </Box>
  );
}