import { memo } from 'react';
import {
  Box,
  VStack,
  Text,
  useColorModeValue,
} from '@chakra-ui/react';

export function ScheduleTab() {
  const textMuted = useColorModeValue('gray.600', 'gray.400');

  return (
    <Box p={6}>
      <VStack spacing={4} align="center" py={16}>
        <Text fontSize="lg" fontWeight="bold" color="brand.500">
          BACKUP SCHEDULE
        </Text>
        <Text color={textMuted}>
          Schedule configuration coming soon...
        </Text>
      </VStack>
    </Box>
  );
}